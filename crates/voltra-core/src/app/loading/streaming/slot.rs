//! Where one region of a streamed world is in its life.

use voltra_ecs::Entity;

use super::super::build::SceneBuild;
use super::super::unload::Unload;

/// One region's state: not in, loading, in with what it spawned, going, or
/// broken (its file would not load: not tried again until the world is
/// streamed anew).
#[derive(Debug)]
pub(super) enum Slot {
    Unloaded,
    Loading(SceneBuild),
    Loaded(Vec<Entity>),
    Unloading(Unload),
    Broken,
}
