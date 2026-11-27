//! Loads that span frames: a full scene change behind a loading screen
//! (`scene_change.rs`), and a big world's regions streamed around its
//! anchors (`streaming.rs`).
//!
//! Stage 30's scene loads, made asynchronous. Both go through one pipeline
//! (`build.rs`): the file read, parsed and checked and its textures decoded
//! on the job pool, then its entities added on the frame within the
//! frame's **load budget** (`budget.rs`) — [`DEFAULT_LOAD_BUDGET`] unless
//! [`App::with_load_budget`] says otherwise, shared by the change and the
//! streaming. Run once a frame, last, after the scene requests the ticks
//! made, so what a region brings is there for the next frame's ticks and
//! this frame's snapshot.
//!
//! **On the web** there are no threads: the pool has no workers, and a
//! task runs when it is first polled — which the loader does only while
//! the budget lasts. The same code, degraded to the main thread a unit at
//! a time; the units that cannot be split (one file's parse, one
//! texture's decode) are each one unit.

mod budget;
mod build;
mod decodes;
mod scene_change;
mod streaming;
mod unload;

#[cfg(test)]
mod tests;

use std::time::Duration;

use voltra_jobs::JobPool;
use voltra_replication::Replication;

pub use budget::{LoadStats, DEFAULT_LOAD_BUDGET};
pub use scene_change::SceneChangeStatus;
pub use streaming::StreamingStats;

use self::budget::FrameBudget;
use self::decodes::Decodes;
use self::scene_change::SceneChange;
use self::streaming::Streaming;
use super::tick::Tick;
use super::App;

/// The most workers loads get unless told otherwise.
#[cfg(not(target_arch = "wasm32"))]
const MAX_LOAD_WORKERS: usize = 4;

/// The pool loads run on, made the first time one starts: a game that
/// never loads over frames starts no threads for it.
///
/// Its own, not the frame's: a load task holds a worker for a whole file,
/// and a frame's scope waiting behind one would be the stutter loading
/// over frames exists to avoid. Fewer workers than cores — all but two, at
/// least one, at most [`MAX_LOAD_WORKERS`] — leaving a core to the frame
/// and one to the rest (the GPU driver, the audio thread): Unreal streams
/// from one loading thread for the same reason. `std` sets no thread
/// priority, so a machine with no spare core shares one between the frame
/// and the loads.
#[derive(Debug, Default)]
struct Pool {
    /// How many workers to make; `None` for the default above.
    workers: Option<usize>,
    pool: Option<JobPool>,
}

impl Pool {
    fn get(&mut self) -> &JobPool {
        let workers = self.workers;
        self.pool
            .get_or_insert_with(|| JobPool::new(workers.unwrap_or_else(default_workers)))
    }
}

/// All cores but two, at least one, at most [`MAX_LOAD_WORKERS`].
#[cfg(not(target_arch = "wasm32"))]
fn default_workers() -> usize {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    cores.saturating_sub(2).clamp(1, MAX_LOAD_WORKERS)
}

/// None: the web has no threads to start.
#[cfg(target_arch = "wasm32")]
fn default_workers() -> usize {
    0
}

/// Everything loading across frames, kept with the scene requests.
#[derive(Debug)]
pub(super) struct Loading {
    pool: Pool,
    budget: Duration,
    decodes: Decodes,
    change: Option<SceneChange>,
    status: Option<SceneChangeStatus>,
    streaming: Streaming,
    stats: LoadStats,
}

impl Default for Loading {
    fn default() -> Self {
        Self {
            pool: Pool::default(),
            budget: DEFAULT_LOAD_BUDGET,
            decodes: Decodes::default(),
            change: None,
            status: None,
            streaming: Streaming::default(),
            stats: LoadStats::default(),
        }
    }
}

impl Loading {
    /// A scene change that sweeps the world at once (`load_scene`): any
    /// change underway is dropped and the streamed world forgotten, its
    /// entities swept with the rest.
    pub(super) fn swept(&mut self) {
        self.change = None;
        if self.status.is_some_and(SceneChangeStatus::is_loading) {
            self.status = None;
        }
        self.streaming.forget();
    }

    /// Asks for `world` — or none — to be streamed.
    pub(super) fn request_streaming(&mut self, world: Option<voltra_assets::AssetPath>) {
        self.streaming.request(world);
    }
}

impl Tick<'_> {
    /// What the last frame's load work cost against its budget.
    pub fn load_stats(&self) -> LoadStats {
        self.scenes.loading.stats
    }
}

impl App {
    /// Gives loads that span frames `budget` of each frame instead of
    /// [`DEFAULT_LOAD_BUDGET`]. More loads a world faster and stutters
    /// sooner; zero still runs one unit a frame.
    pub fn with_load_budget(mut self, budget: Duration) -> Self {
        self.scenes.loading.budget = budget;
        self
    }

    /// Makes the load pool with `workers` threads instead of one per core
    /// but the caller's. Zero runs every load on the main thread within
    /// the budget, as the web does. Read when the first load starts.
    pub fn with_load_workers(mut self, workers: usize) -> Self {
        self.scenes.loading.pool.workers = Some(workers);
        self
    }

    /// [`Tick::load_stats`], from outside a tick.
    pub fn load_stats(&self) -> LoadStats {
        self.scenes.loading.stats
    }

    /// One frame's load work: the scene change first — it is what the
    /// player is waiting on — then the streamed world, with what is left.
    pub(super) fn update_loading(&mut self) {
        let _loading = voltra_profile::scope("loading");
        let mut loading = std::mem::take(&mut self.scenes.loading);
        let mut budget = FrameBudget::new(loading.budget);
        self.advance_scene_change(&mut loading, &mut budget);
        self.advance_streaming(&mut loading, &mut budget);
        loading.stats = budget.finish();
        self.scenes.loading = loading;
    }

    /// Whether this end is a replication client, whose server sends the
    /// replicated entities a load would otherwise make a second copy of.
    fn is_replication_client(&self) -> bool {
        matches!(self.net.replication(), Some(Replication::Client(_)))
    }

    /// On a replication client, lets go of the level entities it set aside
    /// out of interest without the server ever sending them: the level
    /// they came from is going (a scene change).
    fn forget_unsent_level(&mut self) {
        if let Some(client) = self.net.replication_mut().and_then(Replication::client_mut) {
            client.forget_unsent();
        }
    }

    /// [`forget_unsent_level`](Self::forget_unsent_level), of the entities
    /// `ids` names alone: the replicated entities of regions unloading,
    /// while the rest of the level stays.
    fn forget_unsent_of(&mut self, ids: &[voltra_scene::SceneId]) {
        if let Some(client) = self.net.replication_mut().and_then(Replication::client_mut) {
            client.forget_unsent_of(ids);
        }
    }
}
