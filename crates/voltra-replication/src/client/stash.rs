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
//! the session ends. Bounded by what the server knows it culled.

use std::collections::HashMap;

use voltra_ecs::{Entity, World};
use voltra_scene::format::{record_entity, spawn_records, EntityRecord};
use voltra_scene::{hierarchy, ComponentRegistry, Replicated, SceneId};

use crate::NetId;

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
    pub(crate) fn keep(
        &mut self,
        world: &mut World,
        registry: &ComponentRegistry,
        replicated: &[&str],
        entity: Entity,
        net_id: &dyn Fn(Entity) -> Option<NetId>,
    ) {
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
                    if world.get::<Replicated>(child).is_some() {
                        groups.push(child);
                    } else {
                        frontier.push(child);
                    }
                }
            }
            let Some(id) = records.first().map(|record| record.id) else {
                continue;
            };
            let net = net_id(top);
            if let Some(net) = net {
                self.by_net.insert(net, id);
            }
            self.kept.insert(id, (net, records));
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
mod tests {
    use super::*;
    use voltra_scene::{Name, Sprite, Transform};

    #[test]
    fn an_entity_comes_back_as_it_was_but_for_what_the_wire_brings() {
        let registry = ComponentRegistry::with_defaults();
        let mut world = World::new();
        let ship = world.spawn();
        let ship_id = SceneId::new();
        world.insert(ship, ship_id);
        world.insert(ship, Replicated::default());
        world.insert(ship, Transform::default());
        world.insert(ship, Sprite::default().with_sort_order(4));
        let flame = world.spawn();
        world.insert(flame, SceneId::new());
        world.insert(flame, Name::new("flame"));
        hierarchy::set_parent(&mut world, flame, ship).expect("parents");
        let turret = world.spawn();
        let turret_id = SceneId::new();
        world.insert(turret, turret_id);
        world.insert(turret, Replicated::default());
        world.insert(turret, Transform::default());
        hierarchy::set_parent(&mut world, turret, ship).expect("parents");

        let mut stash = Stash::default();
        let net = |e: Entity| (e == turret).then_some(9);
        stash.keep(&mut world, &registry, &["Transform"], ship, &net);
        assert_eq!(world.entity_count(), 0, "despawned, all of it");
        assert_eq!(stash.len(), 2, "the ship and its turret, each its own");

        let back = stash.restore(&mut world, &registry, ship_id).expect("kept");
        assert_eq!(back.len(), 2, "the ship and its flame");
        assert_eq!(
            world.get::<Sprite>(back[0]).map(|s| s.sort_order),
            Some(4),
            "its own state kept"
        );
        assert!(
            world.get::<Transform>(back[0]).is_none(),
            "the replicated one comes with the spawn"
        );
        hierarchy::resolve_parents(&mut world);
        assert_eq!(hierarchy::parent_of(&world, back[1]), Some(back[0]));
        assert!(
            stash.restore(&mut world, &registry, ship_id).is_none(),
            "once"
        );

        stash.forget(9);
        assert_eq!(stash.len(), 0, "the turret despawned for real");
    }
}
