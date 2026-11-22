//! Changing scene, and loading one beside another, from a game's tick.
//!
//! Unity's `SceneManager.LoadScene` in its two modes, and Godot's
//! `change_scene_to_file`. A tick *asks*; the loop does it once the frame's
//! ticks and steps are over — Godot defers the change the same way — so
//! nothing the tick is still holding is despawned under it, and the new
//! scene's first frame is a whole frame.
//!
//! A scene is read through the prefab store, which already turns a scene file
//! into fresh entities with their parents remapped; what this adds is which
//! entities go, and keeping track of what an additive load brought so it can
//! be taken away again.

use std::collections::HashMap;

use voltra_assets::AssetPath;
use voltra_ecs::Entity;
use voltra_scene::{hierarchy, prefab, Persistent};

use super::spawning::prefab_store;
use super::tick::Tick;
use super::App;

/// A scene loaded beside the current one, to unload by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneLoad(u32);

#[derive(Debug, Clone, PartialEq)]
enum Request {
    Replace(AssetPath),
    Add(AssetPath, SceneLoad),
    Unload(SceneLoad),
}

/// What the ticks asked for this frame, and what every additive load brought.
#[derive(Debug, Default)]
pub(super) struct Scenes {
    requests: Vec<Request>,
    next: u32,
    loaded: HashMap<SceneLoad, Vec<Entity>>,
}

impl Tick<'_> {
    /// Replaces the scene with the one at `path`, once this frame is over.
    ///
    /// Every root entity goes, with everything under it, except those marked
    /// [`Persistent`]. A file that will not load changes nothing: the error is
    /// logged and the game stays where it was, rather than in an empty world.
    pub fn load_scene(&mut self, path: &AssetPath) {
        self.scenes.requests.push(Request::Replace(path.clone()));
    }

    /// Loads the scene at `path` beside the current one, once this frame is
    /// over. The handle unloads it again.
    pub fn add_scene(&mut self, path: &AssetPath) -> SceneLoad {
        let load = SceneLoad(self.scenes.next);
        self.scenes.next = self.scenes.next.wrapping_add(1);
        self.scenes.requests.push(Request::Add(path.clone(), load));
        load
    }

    /// Takes away everything `load` brought that is still there, once this
    /// frame is over. A handle already unloaded, or swept away by a scene
    /// change, does nothing.
    pub fn unload_scene(&mut self, load: SceneLoad) {
        self.scenes.requests.push(Request::Unload(load));
    }
}

impl App {
    /// Carries out every scene request the frame's ticks made, in order.
    pub(super) fn apply_scene_requests(&mut self) {
        let requests = std::mem::take(&mut self.scenes.requests);
        let changed = !requests.is_empty();
        for request in requests {
            match request {
                Request::Replace(path) => self.replace_scene(&path),
                Request::Add(path, load) => {
                    if let Some(entities) = self.spawn_scene(&path) {
                        log::info!("added {} ({} entities)", path.as_str(), entities.len());
                        self.scenes.loaded.insert(load, entities);
                    }
                }
                Request::Unload(load) => {
                    let Some(entities) = self.scenes.loaded.remove(&load) else {
                        continue;
                    };
                    for entity in entities {
                        if self.world.is_alive(entity) {
                            hierarchy::despawn_recursive(&mut self.world, entity);
                        }
                    }
                }
            }
        }
        // A scene saved with an older copy of a prefab arrives in step.
        if changed {
            self.sync_all_instances();
        }
        // What the new scenes brought is drawn and heard this very frame.
        self.resolve_spawned();
    }

    /// Spawns the new scene first and only then despawns the old one, so a
    /// scene whose components will not read leaves the old one standing.
    fn replace_scene(&mut self, path: &AssetPath) {
        let Some(arrived) = self.spawn_scene(path) else {
            return;
        };
        let keep: std::collections::HashSet<Entity> = arrived.iter().copied().collect();
        let leaving: Vec<Entity> = self
            .world
            .entities()
            .filter(|entity| {
                !keep.contains(entity)
                    && hierarchy::parent_of(&self.world, *entity).is_none()
                    && self.world.get::<Persistent>(*entity).is_none()
            })
            .collect();
        for entity in leaving {
            if self.world.is_alive(entity) {
                hierarchy::despawn_recursive(&mut self.world, entity);
            }
        }
        // Contacts, sleep and pending events all name a world that is gone;
        // ending every old pair with an event on the next step would report
        // touches the new scene never had.
        self.physics_world.reset();
        self.pending_events.clear();
        self.scenes.loaded.clear();
        log::info!("changed to {} ({} entities)", path.as_str(), arrived.len());
    }

    /// Reads `path` and spawns a copy of it, or logs why not. The entities
    /// are queued for asset resolution.
    fn spawn_scene(&mut self, path: &AssetPath) -> Option<Vec<Entity>> {
        let scenes = prefab_store(&mut self.prefabs, &self.asset_root);
        let handle = scenes.load(path);
        let scene = scenes.get(handle);
        // A prefab with no source is the stand-in for a file that would not
        // read — the store has already said why.
        if scene.source().is_none() {
            log::error!(
                "could not load scene {}; staying in this one",
                path.as_str()
            );
            return None;
        }
        match prefab::spawn_scene(&mut self.world, &self.registry, scene) {
            Ok(instance) => {
                self.spawned.extend_from_slice(&instance.entities);
                Some(instance.entities)
            }
            Err(e) => {
                log::error!(
                    "could not load scene {}: {e}; staying in this one",
                    path.as_str()
                );
                None
            }
        }
    }
}

#[cfg(test)]
mod tests;
