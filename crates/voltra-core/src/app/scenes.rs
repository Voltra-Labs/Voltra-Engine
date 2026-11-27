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

use std::collections::{HashMap, HashSet};

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};
use voltra_scene::{hierarchy, prefab, Persistent, SceneId};

use super::spawning::prefab_store;
use super::tick::Tick;
use super::App;

/// A scene loaded beside the current one, to unload by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneLoad(u32);

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Request {
    Replace(AssetPath),
    Add(AssetPath, SceneLoad),
    Unload(SceneLoad),
    /// A change over frames, with its loading screen (`loading/scene_change.rs`).
    Change(AssetPath, Option<AssetPath>),
    /// A world to stream, or none (`loading/streaming.rs`).
    Stream(Option<AssetPath>),
}

/// What the ticks asked for this frame, what every additive load brought,
/// and the loads that span frames.
#[derive(Debug, Default)]
pub(super) struct Scenes {
    pub(super) requests: Vec<Request>,
    next: u32,
    loaded: HashMap<SceneLoad, Vec<Entity>>,
    pub(super) loading: super::loading::Loading,
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
    /// Starts in the scene at `path`: it replaces whatever the world holds
    /// at the end of the first frame, as a tick's
    /// [`Tick::load_scene`] does, and through the same reads — the asset
    /// pack in a browser. What a game with its own binary opens with.
    pub fn load_scene(&mut self, path: &AssetPath) {
        self.scenes.requests.push(Request::Replace(path.clone()));
    }

    /// Carries out every scene request the frame's ticks made, in order.
    pub(super) fn apply_scene_requests(&mut self) {
        let requests = std::mem::take(&mut self.scenes.requests);
        let changed = !requests.is_empty();
        for request in requests {
            match request {
                Request::Replace(path) => self.replace_scene(&path),
                Request::Add(path, load) => {
                    if let Some(entities) = self.spawn_scene(&path, false) {
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
                Request::Change(path, screen) => self.begin_scene_change(&path, screen.as_ref()),
                Request::Stream(world) => self.scenes.loading.request_streaming(world),
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
    ///
    /// With replication on, the level keeps its file's ids: both ends of a
    /// session must hold each level entity under one id for a client's copy
    /// to be the server's (a dedicated server opens its scene as authored).
    /// The old level holding the same ids for the moment between the spawn
    /// and the sweep is harmless: nothing reads an id in between. A record
    /// whose id a [`Persistent`] entity holds — one that outlives the
    /// sweep — is left out, so no two entities answer to one id.
    fn replace_scene(&mut self, path: &AssetPath) {
        let authored = self.net.replicates();
        let Some(arrived) = self.spawn_scene(path, authored) else {
            return;
        };
        // On a client, what the old level set aside goes with it.
        self.forget_unsent_level();
        let keep: HashSet<Entity> = arrived.iter().copied().collect();
        self.sweep_roots(&keep);
        // What the server had sent of the new level went with the old one.
        self.resync_swept();
        // A change over frames underway, and the streamed world, were for
        // the scene just swept.
        self.scenes.loading.swept();
        log::info!("changed to {} ({} entities)", path.as_str(), arrived.len());
    }

    /// Despawns every root but those in `keep` and the [`Persistent`] ones,
    /// with everything under them: the old scene, on a change. What the
    /// additive loads were tracking is forgotten with it.
    pub(super) fn sweep_roots(&mut self, keep: &HashSet<Entity>) {
        let leaving: Vec<Entity> = self
            .world
            .entities()
            .filter(|entity| {
                !keep.contains(entity)
                    && hierarchy::parent_of(&self.world, *entity).is_none()
                    && self.world.get::<Persistent>(*entity).is_none()
            })
            .collect();
        // One pass over the links for the whole sweep, as an unload makes.
        let mut leaving_set: HashSet<Entity> = leaving.iter().copied().collect();
        let mut children: HashMap<Entity, Vec<Entity>> = HashMap::new();
        for (child, link) in self.world.query::<voltra_scene::Parent>() {
            if let Some(parent) = link.entity {
                children.entry(parent).or_default().push(child);
            }
        }
        let mut frontier = leaving;
        while let Some(entity) = frontier.pop() {
            if let Some(under) = children.get(&entity) {
                frontier.extend(under.iter().copied().filter(|c| leaving_set.insert(*c)));
            }
        }
        for entity in leaving_set {
            self.world.despawn(entity);
        }
        // Contacts, sleep and pending events all name a world that is gone;
        // ending every old pair with an event on the next step would report
        // touches the new scene never had.
        self.physics_world.reset();
        self.pending_physics.clear();
        self.scenes.loaded.clear();
    }

    /// Reads `path` and spawns a copy of it, or logs why not. The entities
    /// are queued for asset resolution.
    ///
    /// `authored` keeps the file's ids rather than giving each entity a
    /// fresh one ([`prefab::spawn_scene_as_authored`]).
    fn spawn_scene(&mut self, path: &AssetPath, authored: bool) -> Option<Vec<Entity>> {
        let scenes = prefab_store(&mut self.prefabs, &self.asset_root, &self.registry);
        let handle = scenes.load_scene(path);
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
        let spawned = if authored {
            let staying = surviving_ids(&self.world);
            prefab::spawn_scene_as_authored(&mut self.world, &self.registry, scene, &|id| {
                staying.contains(&id)
            })
        } else {
            prefab::spawn_scene(&mut self.world, &self.registry, scene)
        };
        match spawned {
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

/// The ids of what a scene change leaves standing: every [`Persistent`]
/// root and what hangs under it.
pub(super) fn surviving_ids(world: &World) -> HashSet<SceneId> {
    world
        .query::<Persistent>()
        .map(|(entity, _)| entity)
        .filter(|&entity| hierarchy::parent_of(world, entity).is_none())
        .flat_map(|root| std::iter::once(root).chain(hierarchy::descendants(world, root)))
        .filter_map(|entity| world.get::<SceneId>(entity).copied())
        .collect()
}

#[cfg(test)]
mod tests;
