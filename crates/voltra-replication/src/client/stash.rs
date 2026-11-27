//! What a client sets aside when an entity goes out of its interest, to
//! put back when it returns.
//!
//! Out of interest, the entity is despawned on the client — Unreal closes
//! the actor's channel and destroys it, and so does this: nothing is left
//! in the world to draw, collide or be found by a query. But only its
//! replicated components come back on the wire. The rest — the sprite a
//! level entity was loaded with, a prefab instance's overrides, the
//! children that only draw — is the client's own and would be lost, so it
//! is kept here as records, by [`SceneId`], and spawned again under the
//! same ids when the server spawns the entity again. Its replicated
//! components are left out of the records: the spawn brings them whole,
//! and a stale one (a component the server removed meanwhile) must not
//! come back.
//!
//! Kept until the entity comes back, the server despawns it for real, or
//! the session ends. What the server culled is bounded by what it knows it
//! culled: a real despawn reaches each. What the first complete snapshot
//! swept (a level entity the server never sent, which under interest may
//! be only far away) has no number the server sends; it is bounded by the
//! level, and let go of when the level goes
//! ([`ReplicationClient::forget_unsent`](crate::ReplicationClient::forget_unsent),
//! a scene change; [`forget_unsent_of`](crate::ReplicationClient::forget_unsent_of),
//! a region unloaded, its own alone — a region's entities have fresh ids
//! the server never sends, so they would otherwise stay).

use std::collections::HashMap;

use voltra_ecs::{Entity, World};
use voltra_scene::format::{record_entity, spawn_records, EntityRecord};
use voltra_scene::{hierarchy, ComponentRegistry, Replicated, SceneId};

use crate::NetId;

/// What [`Stash::keep`] asks of the entities it meets.
pub(crate) struct KeepIds<'a> {
    /// The number the server sends an entity by, if it has one.
    pub(crate) net_id: &'a dyn Fn(Entity) -> Option<NetId>,
    /// Whether a replicated entity stays in the world: the server still
    /// sends it.
    pub(crate) stays: &'a dyn Fn(Entity) -> bool,
}

/// The records of a replicated entity set aside, and the number the server
/// sent it by, when it had one.
#[derive(Debug, Default)]
pub(crate) struct Stash {
    kept: HashMap<SceneId, (Option<NetId>, Vec<EntityRecord>)>,
    by_net: HashMap<NetId, SceneId>,
}

impl Stash {
    /// Records `entity` and what hangs under it, then despawns it all.
    /// Each replicated entity in the tree is kept on its own, under its own
    /// id (`net_id` names the server's numbers), so each comes back with
    /// its own spawn; a non-replicated one goes with the replicated entity
    /// above it. `replicated` names the components the wire brings.
    ///
    /// A replicated descendant for which `stays` answers true — one the
    /// server still sends this client — is neither kept nor despawned: it
    /// is unparented, with what hangs under it, and left in the world.
    /// The hierarchy is not replicated, so the client's tree can be one the
    /// server has since changed (a turret moved to another tank), and the
    /// server is the one that says what is out of interest.
    pub(crate) fn keep(
        &mut self,
        world: &mut World,
        registry: &ComponentRegistry,
        replicated: &[&str],
        entity: Entity,
        ids: &KeepIds<'_>,
    ) {
        let mut staying = Vec::new();
        let mut groups: Vec<Entity> = vec![entity];
        while let Some(top) = groups.pop() {
            let mut records = Vec::new();
            let mut frontier = vec![top];
            while let Some(current) = frontier.pop() {
                match record_entity(world, registry, current) {
                    Some(Ok(mut record)) => {
                        if current == top {
                            for name in replicated {
                                record.components.remove(*name);
                            }
                        }
                        records.push(record);
                    }
                    Some(Err(error)) => {
                        log::warn!("an entity out of interest would not record: {error}");
                    }
                    // No id: transient, as a save leaves it out.
                    None => {}
                }
                if records.is_empty() {
                    // The entity itself could not be kept: it comes back
                    // as a spawn without its records would, and what hung
                    // under it goes with it.
                    break;
                }
                for child in hierarchy::children_of(world, current) {
                    if world.get::<Replicated>(child).is_none() {
                        frontier.push(child);
                    } else if (ids.stays)(child) {
                        staying.push(child);
                    } else {
                        groups.push(child);
                    }
                }
            }
            let Some(id) = records.first().map(|record| record.id) else {
                continue;
            };
            let net = (ids.net_id)(top);
            if let Some(net) = net {
                self.by_net.insert(net, id);
            }
            self.kept.insert(id, (net, records));
        }
        for child in staying {
            hierarchy::clear_parent(world, child);
        }
        hierarchy::despawn_recursive(world, entity);
    }

    /// Spawns back what was kept under `id`, if anything was: the entity
    /// first, then what hung under it. Parents are left for the caller to
    /// resolve once, after every spawn of a snapshot. `None` when nothing
    /// was kept, or what was would not spawn (warned).
    pub(crate) fn restore(
        &mut self,
        world: &mut World,
        registry: &ComponentRegistry,
        id: SceneId,
    ) -> Option<Vec<Entity>> {
        let (net, records) = self.kept.remove(&id)?;
        if let Some(net) = net {
            self.by_net.remove(&net);
        }
        match spawn_records(&records, registry, world) {
            Ok(entities) if !entities.is_empty() => Some(entities),
            Ok(_) => None,
            Err(error) => {
                log::warn!("an entity back in interest would not spawn as it was: {error}");
                None
            }
        }
    }

    /// Lets go of what the server despawned for real while it was out.
    pub(crate) fn forget(&mut self, net: NetId) {
        if let Some(id) = self.by_net.remove(&net) {
            self.kept.remove(&id);
        }
    }

    /// Lets go of what was set aside with no number the server sends it by:
    /// the level entities swept, never sent.
    pub(crate) fn forget_unnumbered(&mut self) {
        self.kept.retain(|_, (net, _)| net.is_some());
    }

    /// [`forget_unnumbered`](Self::forget_unnumbered), of `ids` alone: the
    /// entities of one region of several.
    pub(crate) fn forget_unnumbered_of(&mut self, ids: &[SceneId]) {
        for id in ids {
            if let Some((None, _)) = self.kept.get(id) {
                self.kept.remove(id);
            }
        }
    }

    /// Lets go of everything: a new session.
    pub(crate) fn clear(&mut self) {
        self.kept.clear();
        self.by_net.clear();
    }

    /// How many entities are set aside.
    pub(crate) fn len(&self) -> usize {
        self.kept.len()
    }
}

#[cfg(test)]
mod tests;
