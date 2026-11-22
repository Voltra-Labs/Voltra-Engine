//! Where the editor keeps the scene, and saving it there.

use std::path::{Path, PathBuf};

use voltra_ecs::World;
use voltra_scene::ComponentRegistry;

/// Where the editor saves when no path has been chosen.
///
/// A parameter with a default rather than a value baked into `save` and
/// `load` — the second caller is a file dialog, and it is already
/// foreseeable.
pub(crate) fn default_path() -> &'static Path {
    Path::new("assets/scenes/scene.ron")
}

/// Writes `world` to the scene file, making its directory first. Answers
/// where it went, or why it did not.
pub(crate) fn save(world: &World, registry: &ComponentRegistry) -> Result<PathBuf, String> {
    let path = default_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
    }
    voltra_scene::format::save(world, registry, path)
        .map_err(|e| format!("could not save the scene: {e}"))?;
    Ok(path.to_path_buf())
}
