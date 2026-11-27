//! The world read once a snapshot: every replicated entity's spawn and
//! components as they are now, and the entities gone since the last read.

use std::collections::HashMap;
use std::sync::Arc;

use voltra_ecs::{Entity, World};
use voltra_scene::{ComponentRegistry, PrefabInstance, Replicated, SceneId};

use super::snapshot::Tracked;
use super::view::Value;
use super::ReplicationServer;
use crate::wire::{self, NetId, Spawn};
use crate::NetOwner;

impl ReplicationServer {
    /// Reads the world: every replicated entity's spawn and components as
    /// they are now, and the entities gone since the last read.
    pub(super) fn refresh(&mut self, world: &mut World, registry: &ComponentRegistry) {
        let live: Vec<(Entity, f32)> = world
            .query::<Replicated>()
            .map(|(entity, replicated)| (entity, replicated.priority))
            .collect();

        let mut seen = HashMap::with_capacity(live.len());
        for (entity, priority) in live {
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
                },
            );
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
                // Only a client that may have it needs telling.
                if let Some(entity) = view.entities.remove(&id) {
                    if entity.sent_spawn.is_some() {
                        view.despawned.insert(id);
                    }
                }
            }
        }
    }
}
