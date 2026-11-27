//! Where one region of a streamed world is in its life.

use voltra_ecs::{Entity, World};
use voltra_scene::{Replicated, SceneId};

use super::super::build::SceneBuild;
use super::super::unload::Unload;

/// One region's state: not in, loading, in with what it spawned, going, or
/// broken (its file would not load: not tried again until the world is
/// streamed anew).
#[derive(Debug)]
pub(super) enum Slot {
    Unloaded,
    Loading(SceneBuild),
    /// What the region spawned, and the ids of its replicated entities: on
    /// a replication client, what a first snapshot swept of them is let go
    /// of when the region goes, and nothing else (`forget_unsent_of`).
    Loaded(Vec<Entity>, Vec<SceneId>),
    Unloading(Unload),
    Broken,
}

/// The ids of the replicated entities among `entities` still in `world`.
pub(super) fn replicated_ids(world: &World, entities: &[Entity]) -> Vec<SceneId> {
    entities
        .iter()
        .filter(|&&entity| world.get::<Replicated>(entity).is_some())
        .filter_map(|&entity| world.get::<SceneId>(entity).copied())
        .collect()
}
