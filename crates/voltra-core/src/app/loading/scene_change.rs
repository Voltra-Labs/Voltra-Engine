//! A full scene change over frames, behind a loading screen.
//!
//! [`Tick::load_scene`] changes scene in one frame — right for a small
//! level, a stutter for a big one. [`Tick::change_scene`] is Unity's
//! `LoadSceneAsync` in single mode and Godot's `load_threaded_request`:
//!
//! 1. **The loading screen goes up** at the end of the frame that asked —
//!    a prefab, spawned at once (keep it small; it is read on the spot)
//!    — over the old scene, and the physics stops stepping.
//! 2. **The new file is read and checked on the pool.** One that will not
//!    load takes the screen down and leaves the old scene standing, as
//!    `load_scene` does ([`SceneChangeStatus::Failed`]).
//! 3. **The old scene goes** — every root but the [`Persistent`] ones and
//!    the screen, in one frame — and the new one comes in within each
//!    frame's load budget, textures first.
//! 4. **The screen comes down** the frame the last entity is in, and the
//!    physics starts again on a fresh world.
//!
//! A tick reads [`Tick::scene_change`] for a progress bar: `Loading` from 0
//! the frame after the request, rising, then `Finished` (progress 1) for
//! one frame, then `None`. The per-frame tick runs throughout — it draws
//! the bar — but the steps, and with them the fixed tick, wait: a level
//! half spawned is a floor half there.
//!
//! [`Persistent`]: voltra_scene::Persistent

use std::collections::HashSet;

use voltra_assets::AssetPath;
use voltra_ecs::Entity;
use voltra_scene::{hierarchy, prefab, Persistent, Transform2D};

use super::budget::FrameBudget;
use super::build::{Replicas, SceneBuild, Step};
use super::streaming::Streaming;
use super::Loading;
use crate::app::scenes::Request;
use crate::app::spawning::prefab_store;
use crate::app::tick::Tick;
use crate::app::App;

/// How a change asked for with [`Tick::change_scene`] is going.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SceneChangeStatus {
    /// Underway: `progress` runs from 0 to 1.
    Loading {
        /// How far along, from 0 to 1.
        progress: f32,
    },
    /// The new scene is in and the loading screen gone, this frame.
    Finished,
    /// The file would not load; the old scene is still standing.
    Failed,
}

impl SceneChangeStatus {
    /// How far along, from 0 to 1: 1 once finished, 0 for a failure.
    pub fn progress(self) -> f32 {
        match self {
            Self::Loading { progress } => progress,
            Self::Finished => 1.0,
            Self::Failed => 0.0,
        }
    }

    /// Whether the change is still underway.
    pub fn is_loading(self) -> bool {
        matches!(self, Self::Loading { .. })
    }
}

/// A change underway.
#[derive(Debug)]
pub(super) struct SceneChange {
    build: SceneBuild,
    /// The loading screen's entities, which the sweep spares.
    screen: Vec<Entity>,
    /// Whether the old scene has gone yet.
    swept: bool,
    /// Whether a frame has passed since the screen went up: the frame
    /// that asked has paid for the screen, and the first tick to see the
    /// change sees it at 0.
    started: bool,
}

impl Tick<'_> {
    /// Changes to the scene at `path` over the frames to come, with the
    /// prefab at `loading` on screen until it is in — see the module.
    /// A change asked for while another is underway replaces it.
    pub fn change_scene(&mut self, path: &AssetPath, loading: Option<&AssetPath>) {
        self.scenes
            .requests
            .push(Request::Change(path.clone(), loading.cloned()));
    }

    /// How the last [`change_scene`](Self::change_scene) is going: `None`
    /// when none is underway or just ended.
    pub fn scene_change(&self) -> Option<SceneChangeStatus> {
        self.scenes.loading.status
    }
}

impl App {
    /// [`Tick::change_scene`], from outside a tick: what a game opening on
    /// a big level starts with.
    pub fn change_scene(&mut self, path: &AssetPath, loading: Option<&AssetPath>) {
        self.scenes
            .requests
            .push(Request::Change(path.clone(), loading.cloned()));
    }

    /// [`Tick::scene_change`], from outside a tick.
    pub fn scene_change(&self) -> Option<SceneChangeStatus> {
        self.scenes.loading.status
    }

    /// Whether a change is underway, which holds the physics still.
    pub(crate) fn is_changing_scene(&self) -> bool {
        self.scenes.loading.change.is_some()
    }

    /// Puts the loading screen up and starts reading `path`, dropping any
    /// change already underway.
    pub(in crate::app) fn begin_scene_change(
        &mut self,
        path: &AssetPath,
        loading_screen: Option<&AssetPath>,
    ) {
        if let Some(previous) = self.scenes.loading.change.take() {
            log::info!(
                "scene change to {} replaced by one to {}",
                previous.build.path().as_str(),
                path.as_str()
            );
            self.take_down(&previous.screen);
            let entities = previous.build.entities().to_vec();
            self.take_down(&entities);
        }
        let screen = match loading_screen {
            Some(screen) => self.spawn_loading_screen(screen),
            None => Vec::new(),
        };
        let root = prefab_store(&mut self.prefabs, &self.asset_root, &self.registry)
            .root()
            .to_path_buf();
        let registry = self.registry.clone();
        // A networked session's level is held under its file's ids on both
        // ends; a client sets its replicated entities aside for the
        // server's copies to be spawned from as they arrive.
        let replicas = if self.is_replication_client() {
            Replicas::SetAside
        } else {
            Replicas::Spawn
        };
        let keep_ids = self
            .net
            .replicates()
            .then(|| crate::app::scenes::surviving_ids(&self.world));
        self.forget_unsent_level();
        let build = SceneBuild::start(
            self.scenes.loading.pool.get(),
            root,
            path,
            registry,
            replicas,
            keep_ids,
        );
        self.scenes.loading.change = Some(SceneChange {
            build,
            screen,
            swept: false,
            started: false,
        });
        self.scenes.loading.status = Some(SceneChangeStatus::Loading { progress: 0.0 });
    }

    /// Runs the change's units while the frame's budget lasts.
    pub(super) fn advance_scene_change(&mut self, loading: &mut Loading, budget: &mut FrameBudget) {
        // A finish or a failure is read for one frame, then forgotten.
        if loading.change.is_none() {
            if !loading.status.is_some_and(SceneChangeStatus::is_loading) {
                loading.status = None;
            }
            return;
        }
        let Some(mut change) = loading.change.take() else {
            return;
        };
        if !change.started {
            change.started = true;
            loading.change = Some(change);
            return;
        }
        let jobs = loading.pool.get();
        let mut outcome = Step::Worked;
        while budget.has_time() {
            let unit = budget.begin();
            outcome = self.step_build(&mut change.build, &mut loading.decodes, jobs);
            if !change.swept && change.build.is_read() {
                self.sweep_for_change(&change.screen, &mut loading.streaming);
                change.swept = true;
                // After the sweep, which a server copy of the old level
                // that arrived early went with (it is sent again).
                let set_aside = change.build.take_set_aside();
                self.keep_level(set_aside);
                self.resync_swept();
            }
            budget.end(unit);
            if outcome != Step::Worked {
                break;
            }
        }
        match outcome {
            Step::Done => {
                self.take_down(&change.screen);
                log::info!(
                    "changed to {} ({} entities)",
                    change.build.path().as_str(),
                    change.build.entities().len()
                );
                loading.status = Some(SceneChangeStatus::Finished);
            }
            Step::Failed => {
                self.take_down(&change.screen);
                if change.swept {
                    log::error!(
                        "{} failed half way in; the old scene is already gone",
                        change.build.path().as_str()
                    );
                } else {
                    log::error!("staying in this scene");
                }
                loading.status = Some(SceneChangeStatus::Failed);
            }
            Step::Worked | Step::Waiting => {
                loading.status = Some(SceneChangeStatus::Loading {
                    progress: change.build.progress(),
                });
                loading.change = Some(change);
            }
        }
    }

    /// Takes the old scene away once the new one is known to load: every
    /// root but the persistent ones and the loading screen's, and what the
    /// old scene's loads and streaming were tracking.
    fn sweep_for_change(&mut self, screen: &[Entity], streaming: &mut Streaming) {
        let keep: HashSet<Entity> = screen.iter().copied().collect();
        self.sweep_roots(&keep);
        streaming.forget();
    }

    /// Spawns the loading screen prefab, its assets resolved this frame.
    fn spawn_loading_screen(&mut self, path: &AssetPath) -> Vec<Entity> {
        let prefabs = prefab_store(&mut self.prefabs, &self.asset_root, &self.registry);
        let handle = prefabs.load(path);
        match prefab::spawn(
            &mut self.world,
            &self.registry,
            prefabs.get(handle),
            &Transform2D::default(),
        ) {
            Ok(instance) => {
                self.spawned.extend_from_slice(&instance.entities);
                instance.roots
            }
            Err(error) => {
                log::error!(
                    "loading screen {} would not spawn: {error}; loading without one",
                    path.as_str()
                );
                Vec::new()
            }
        }
    }

    /// Despawns `roots` and what is under them, but for a root marked
    /// persistent — a loading screen, or what a replaced change had brought
    /// so far.
    fn take_down(&mut self, roots: &[Entity]) {
        for &root in roots {
            if self.world.is_alive(root) && self.world.get::<Persistent>(root).is_none() {
                hierarchy::despawn_recursive(&mut self.world, root);
            }
        }
    }
}
