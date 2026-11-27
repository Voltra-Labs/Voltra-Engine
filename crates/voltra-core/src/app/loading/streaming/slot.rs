//! Where one region of a streamed world is in its life.

use voltra_ecs::Entity;
use voltra_scene::SceneId;

use super::super::build::SceneBuild;
use super::super::unload::Unload;

/// One region's state: not in, loading, in with what it spawned, going, or
/// broken (its file would not load: not tried again until the world is
/// streamed anew).
#[derive(Debug)]
pub(super) enum Slot {
    Unloaded,
    Loading(Box<SceneBuild>),
    /// What the region spawned, and the ids of its replicated entities,
    /// noted as each landed: on a replication client, what a first snapshot
    /// swept of them is let go of when the region goes, and nothing else
    /// (`forget_unsent_of`).
    Loaded(Vec<Entity>, Vec<SceneId>),
    Unloading(Unload),
    Broken,
}
