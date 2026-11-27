//! The world read once a snapshot: every replicated entity's spawn and
//! components as they are now, and the entities gone since the last read.

use std::collections::HashMap;
use std::sync::Arc;

use voltra_ecs::{Entity, World};
use voltra_scene::{hierarchy, ComponentRegistry, PrefabInstance, Replicated, SceneId, Transform};

use super::snapshot::{Place, Tracked};
use super::view::Value;
use super::ReplicationServer;
use crate::wire::{self, NetId, Spawn};
use crate::NetOwner;

impl ReplicationServer {
    /// Reads the world: every replicated entity's spawn and components as
    /// they are now, and the entities gone since the last read.
    pub(super) fn refresh(&mut self, world: &mut World, registry: &ComponentRegistry) {
        let live: Vec<(Entity, Replicated)> = world
            .query::<Replicated>()
            .map(|(entity, replicated)| (entity, *replicated))
            .collect();
        let placed = self.interest.is_some();

        let mut seen = HashMap::with_capacity(live.len());
        for &(entity, replicated) in &live {
            let priority = replicated.priority;
            let id = match self.ids.get(&entity) {
                Some(&id) => id,
                None => {
                    let id = self.next_id;
                    self.next_id = self.next_id.wrapping_add(1).max(1);
                    self.ids.insert(entity, id);
                    id
                }
            };
            seen.insert(id, ());
            let scene_id = match world.get::<SceneId>(entity) {
                Some(&id) => id,
                None => {
                    let fresh = SceneId::new();
                    world.insert(entity, fresh);
                    fresh
                }
            };
            let spawn_value = Spawn {
                id: scene_id,
                prefab: world
                    .get::<PrefabInstance>(entity)
                    .map(|p| p.source.as_str().to_owned()),
                owner: world.get::<NetOwner>(entity).map(|o| o.0 .0),
            };
            let spawn_bytes = wire::encode(&spawn_value);
            let previous = self.tracked.get(&id);
            let spawn = match previous {
                Some(p) if *p.spawn == *spawn_bytes => p.spawn.clone(),
                _ => Arc::from(spawn_bytes),
            };
            let components: Vec<Value> = self
                .components
                .iter()
                .enumerate()
                .map(|(index, &component)| {
                    let bytes = match registry.encode_replicated(component, world, entity)? {
                        Ok(bytes) => bytes,
                        Err(error) => {
                            log::warn!("{} would not encode: {error}", component.name);
                            return None;
                        }
                    };
                    // The previous Arc when unchanged: views compare by value,
                    // and sharing keeps a still world from allocating.
                    match previous.and_then(|p| p.components[index].as_ref()) {
                        Some(old) if **old == *bytes => Some(old.clone()),
                        _ => Some(Arc::from(bytes)),
                    }
                })
                .collect();
            self.tracked.insert(
                id,
                Tracked {
                    entity,
                    spawn,
                    spawn_value,
                    components,
                    priority,
                    place: Place {
                        root: id,
                        at: None,
                        always: replicated.always_relevant,
                        radius: replicated.relevance_radius,
                    },
                },
            );
        }
        if placed {
            // Every live entity has its id now, so a root can be named.
            for &(entity, _) in &live {
                self.place(world, entity);
            }
        }

        let gone: Vec<NetId> = self
            .tracked
            .keys()
            .copied()
            .filter(|id| !seen.contains_key(id))
            .collect();
        for id in gone {
            if let Some(tracked) = self.tracked.remove(&id) {
                self.ids.remove(&tracked.entity);
            }
            for view in self.clients.values_mut() {
                // Only a client that may have it needs telling: one sent
                // its spawn, or one told it went out of interest — which
                // set it aside, and must let go of it.
                let had = view
                    .entities
                    .remove(&id)
                    .is_some_and(|entity| entity.sent_spawn.is_some());
                let culled = view.interest.culled.remove(&id);
                let out = view.interest.out.remove(&id);
                view.interest.roots.remove(&id);
                if had || culled || out {
                    view.despawned.insert(id);
                }
            }
        }
    }

    /// Where `entity` stands: its replicated root and its world position.
    fn place(&mut self, world: &World, entity: Entity) {
        let Some(&id) = self.ids.get(&entity) else {
            return;
        };
        let root = hierarchy::ancestors(world, entity)
            .into_iter()
            .rev()
            .find(|&above| world.get::<Replicated>(above).is_some())
            .and_then(|above| self.ids.get(&above).copied())
            .unwrap_or(id);
        let at = world
            .get::<Transform>(entity)
            .is_some()
            .then(|| hierarchy::world_transform(world, entity).translation);
        if let Some(tracked) = self.tracked.get_mut(&id) {
            tracked.place.root = root;
            tracked.place.at = at;
        }
    }
}
