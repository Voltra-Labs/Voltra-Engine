//! What a streamed world reports of itself.

use voltra_assets::AssetPath;

use super::{Slot, Streaming};

/// Where a streamed world stands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StreamingStats {
    /// The world file being streamed, if one is.
    pub world: Option<AssetPath>,
    /// How many regions it has.
    pub regions: usize,
    /// How many are in.
    pub loaded: usize,
    /// How many are on their way in.
    pub loading: usize,
    /// How many are on their way out, a stopped world's included.
    pub unloading: usize,
    /// How many would not load, and are not retried.
    pub broken: usize,
    /// How many entities the loaded regions brought.
    pub entities: usize,
}

pub(super) fn stats_of(streaming: &Streaming) -> StreamingStats {
    let mut stats = StreamingStats {
        unloading: streaming.orphans.len(),
        ..StreamingStats::default()
    };
    if let Some(world) = &streaming.world {
        stats.world = Some(world.path.clone());
        stats.regions = world.slots.len();
        for slot in &world.slots {
            match slot {
                Slot::Unloaded => {}
                Slot::Loading(_) => stats.loading += 1,
                Slot::Loaded(entities, _) => {
                    stats.loaded += 1;
                    stats.entities += entities.len();
                }
                Slot::Unloading(_) => stats.unloading += 1,
                Slot::Broken => stats.broken += 1,
            }
        }
    }
    stats
}
