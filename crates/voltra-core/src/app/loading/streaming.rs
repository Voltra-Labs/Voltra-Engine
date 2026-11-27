//! A big world's regions loaded and unloaded around its anchors.
//!
//! [`Tick::stream_world`] names a `.world.ron` ([`WorldRegions`]); from the
//! end of that frame on, every frame:
//!
//! - **Anchors** are found: every [`StreamingAnchor`] — the players of a
//!   server, a split screen's two — or, with none, every active camera.
//!   Each keeps the regions within its radius in.
//! - **A region within an anchor's radius loads**, nearest first, a few at
//!   a time: read, checked and its textures decoded on the pool, then its
//!   entities added within the frame's budget (`build.rs`).
//! - **A region further than radius plus margin from every anchor
//!   unloads**, its entities despawned within the budget too
//!   (`unload.rs`). The margin is the hysteresis that keeps an anchor
//!   pacing along an edge from loading a region every other frame. One
//!   that stops being wanted while still loading is cancelled: an unstarted
//!   read never runs, a half-spawned region is unloaded.
//! - Loads and unloads take turns within the budget, so neither starves.
//!
//! **What a region owns** is what it spawned: an entity that walks out of
//! its region is despawned with it all the same, as an actor is with its
//! streamed level in Unreal. A [`Persistent`] one is lifted out instead.
//! Region state does not survive an unload — a reloaded region is the
//! file again; what the player changed is the game's to save.
//!
//! **Replication.** A server streams around every player it hosts: give
//! each player entity a [`StreamingAnchor`]. A replicated entity in a
//! region is the server's — spawned and despawned with the region there,
//! and so on every client. A *client* streams its own regions around its
//! camera for the scenery, but **leaves out every replicated entity** a
//! region holds: the server's copy arrives through replication, and a
//! local one would be a double the server never sent. (Place replicated
//! things in a region as prefab instances, so a client can build them
//! from the prefab the server names.)
//!
//! [`Persistent`]: voltra_scene::Persistent

mod anchors;
mod stats;

use voltra_assets::AssetPath;
use voltra_ecs::Entity;
use voltra_scene::WorldRegions;

use super::budget::FrameBudget;
use super::build::{SceneBuild, Step};
use super::unload::Unload;
use super::Loading;
use crate::app::scenes::Request;
use crate::app::spawning::prefab_store;
use crate::app::tick::Tick;
use crate::app::App;

use stats::stats_of;
pub use stats::StreamingStats;

/// How many regions may be loading at once, per thread of the pool: enough
/// to keep the workers fed, few enough that a fast anchor does not queue a
/// world's worth of reads it will have left behind by the time they run.
const LOADS_PER_THREAD: usize = 2;

#[derive(Debug)]
enum Slot {
    Unloaded,
    Loading(SceneBuild),
    Loaded(Vec<Entity>),
    Unloading(Unload),
    Broken,
}

#[derive(Debug)]
struct StreamedWorld {
    path: AssetPath,
    regions: WorldRegions,
    slots: Vec<Slot>,
}

/// The world being streamed, and what a stopped one left to unload.
#[derive(Debug, Default)]
pub(super) struct Streaming {
    world: Option<StreamedWorld>,
    /// A `stream_world` or `stop_streaming` not applied yet: held while a
    /// scene change is underway, which it is for the scene to come.
    pending: Option<Option<AssetPath>>,
    /// The regions of a world no longer streamed, going.
    orphans: Vec<Unload>,
    stats: StreamingStats,
}

impl Streaming {
    /// Forgets every region, without despawning: a scene change has swept
    /// them. A request still pending stays, for the new scene.
    pub(super) fn forget(&mut self) {
        self.world = None;
        self.orphans.clear();
    }

    /// Asks for `world` — or for none — to be streamed from the next update.
    pub(super) fn request(&mut self, world: Option<AssetPath>) {
        self.pending = Some(world);
    }

    pub(super) fn stats(&self) -> &StreamingStats {
        &self.stats
    }
}

impl Tick<'_> {
    /// Streams the world `path` describes around the scene's anchors, from
    /// the end of this frame — see the module. A world already streaming is
    /// unloaded first; one asked for during a [`change_scene`] waits for
    /// the new scene.
    ///
    /// [`change_scene`]: Tick::change_scene
    pub fn stream_world(&mut self, path: &AssetPath) {
        self.scenes
            .requests
            .push(Request::Stream(Some(path.clone())));
    }

    /// Stops streaming, unloading every region over the frames to come.
    pub fn stop_streaming(&mut self) {
        self.scenes.requests.push(Request::Stream(None));
    }

    /// Where the streamed world stands, as of the end of the last frame.
    pub fn streaming(&self) -> &StreamingStats {
        self.scenes.loading.streaming.stats()
    }
}

impl App {
    /// [`Tick::stream_world`], from outside a tick.
    pub fn stream_world(&mut self, path: &AssetPath) {
        self.scenes
            .requests
            .push(Request::Stream(Some(path.clone())));
    }

    /// [`Tick::stop_streaming`], from outside a tick.
    pub fn stop_streaming(&mut self) {
        self.scenes.requests.push(Request::Stream(None));
    }

    /// [`Tick::streaming`], from outside a tick.
    pub fn streaming(&self) -> &StreamingStats {
        self.scenes.loading.streaming.stats()
    }

    /// Runs the streamed world's units while the frame's budget lasts.
    pub(super) fn advance_streaming(&mut self, loading: &mut Loading, budget: &mut FrameBudget) {
        let changing = loading.change.is_some();
        if !changing {
            if let Some(next) = loading.streaming.pending.take() {
                self.switch_world(&mut loading.streaming, next);
            }
        }
        let streaming = &mut loading.streaming;
        let jobs = loading.pool.get();
        // A scene change is taking the world away: no region starts or
        // stops until it is done, but a stopped world's go on going.
        let mut order: Vec<(f32, usize)> = Vec::new();
        let mut unloaded = false;
        if let (Some(world), false) = (streaming.world.as_mut(), changing) {
            let anchors = self.streaming_anchors(world.regions.load_radius);
            let margin = world.regions.unload_margin;
            let mut starts: Vec<(f32, usize)> = Vec::new();
            let mut active = 0;
            for (index, region) in world.regions.regions.iter().enumerate() {
                // How far beyond the nearest anchor's reach: zero or less
                // is wanted.
                let reach = anchors
                    .iter()
                    .map(|(at, radius)| region.distance_to(*at) - radius)
                    .fold(f32::INFINITY, f32::min);
                let wanted = reach <= 0.0;
                let kept = reach <= margin;
                let slot = &mut world.slots[index];
                match slot {
                    Slot::Unloaded if wanted => starts.push((reach, index)),
                    // Cancelled: a read not started never runs, and what a
                    // spawn put in so far goes.
                    Slot::Loading(build) if !kept => {
                        let entities = build.entities().to_vec();
                        *slot = if entities.is_empty() {
                            Slot::Unloaded
                        } else {
                            Slot::Unloading(Unload::new(entities))
                        };
                    }
                    Slot::Loaded(entities) if !kept => {
                        unloaded = true;
                        *slot = Slot::Unloading(Unload::new(std::mem::take(entities)));
                    }
                    _ => {}
                }
                if let Slot::Loading(_) = world.slots[index] {
                    active += 1;
                    order.push((reach, index));
                }
            }
            // Nearest first, as many as the pool can usefully take.
            starts.sort_by(|a, b| a.0.total_cmp(&b.0));
            let limit = (jobs.threads() * LOADS_PER_THREAD).max(LOADS_PER_THREAD);
            let root = prefab_store(&mut self.prefabs, &self.asset_root, &self.registry)
                .root()
                .to_path_buf();
            let skip = self.is_replication_client();
            for (reach, index) in starts.into_iter().take(limit.saturating_sub(active)) {
                let scene = &world.regions.regions[index].scene;
                let build =
                    SceneBuild::start(jobs, root.clone(), scene, self.registry.clone(), skip);
                world.slots[index] = Slot::Loading(build);
                order.push((reach, index));
            }
            order.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        if unloaded {
            self.forget_unsent_level();
        }

        // Loads, nearest first, and unloads take turns.
        let mut blocked = vec![false; order.len()];
        loop {
            let mut progressed = false;
            if !budget.has_time() {
                break;
            }
            if let Some(world) = streaming.world.as_mut() {
                if let Some(turn) = blocked.iter().position(|b| !b) {
                    let index = order[turn].1;
                    let unit = budget.begin();
                    let step = match &mut world.slots[index] {
                        Slot::Loading(build) => self.step_build(build, &mut loading.decodes, jobs),
                        _ => Step::Done,
                    };
                    budget.end(unit);
                    match step {
                        Step::Worked => {}
                        Step::Waiting => blocked[turn] = true,
                        Step::Done => {
                            blocked[turn] = true;
                            if let Slot::Loading(build) =
                                std::mem::replace(&mut world.slots[index], Slot::Unloaded)
                            {
                                world.slots[index] = Slot::Loaded(build.into_entities());
                            }
                        }
                        Step::Failed => {
                            blocked[turn] = true;
                            world.slots[index] = Slot::Broken;
                        }
                    }
                    progressed = true;
                }
            }
            if !budget.has_time() {
                break;
            }
            if let Some(unload) = streaming.orphans.last_mut() {
                let unit = budget.begin();
                let done = unload.step(&mut self.world);
                budget.end(unit);
                if done {
                    streaming.orphans.pop();
                }
                progressed = true;
            } else if let Some(world) = streaming.world.as_mut() {
                if let Some(slot) = world
                    .slots
                    .iter_mut()
                    .find(|slot| matches!(slot, Slot::Unloading(_)))
                {
                    let unit = budget.begin();
                    if let Slot::Unloading(unload) = slot {
                        if unload.step(&mut self.world) {
                            *slot = Slot::Unloaded;
                        }
                    }
                    budget.end(unit);
                    progressed = true;
                }
            }
            if !progressed {
                break;
            }
        }
        streaming.stats = stats_of(streaming);
    }

    /// Stops streaming the current world, if any — its regions going over
    /// the frames to come — and starts on `next`, if it reads.
    fn switch_world(&mut self, streaming: &mut Streaming, next: Option<AssetPath>) {
        if let Some(old) = streaming.world.take() {
            for slot in old.slots {
                match slot {
                    Slot::Loading(build) => {
                        streaming
                            .orphans
                            .push(Unload::new(build.entities().to_vec()));
                    }
                    Slot::Loaded(entities) => streaming.orphans.push(Unload::new(entities)),
                    Slot::Unloading(unload) => streaming.orphans.push(unload),
                    Slot::Unloaded | Slot::Broken => {}
                }
            }
            log::info!("stopped streaming {}", old.path.as_str());
        }
        let Some(path) = next else {
            return;
        };
        let root = prefab_store(&mut self.prefabs, &self.asset_root, &self.registry)
            .root()
            .to_path_buf();
        match WorldRegions::read(&root, &path) {
            Ok(regions) => {
                log::info!(
                    "streaming {} ({} regions)",
                    path.as_str(),
                    regions.regions.len()
                );
                let slots = regions.regions.iter().map(|_| Slot::Unloaded).collect();
                streaming.world = Some(StreamedWorld {
                    path,
                    regions,
                    slots,
                });
            }
            Err(error) => log::error!("could not stream {}: {error}", path.as_str()),
        }
    }
}
