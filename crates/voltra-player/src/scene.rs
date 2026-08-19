//! Getting the scene into a world, before there is a window to draw it in.
//!
//! Loading first and opening the window second is deliberate: a scene that
//! cannot be read is the one failure a player cannot recover from, and every
//! engine says so before it shows anything. Godot refuses to start with
//! "Cannot load main scene"; a window that opens onto an empty world would
//! instead look like a working build of a broken game.

use std::path::Path;

use voltra_ecs::World;
use voltra_scene::{camera, ComponentRegistry, SceneError};

/// Reads `path` into a fresh world.
///
/// The registry is the default one: a player that knew about fewer components
/// than the editor saves would drop them silently on load, which is the one
/// way a scene file can lie.
pub fn load(path: &Path) -> Result<World, SceneError> {
    let mut world = World::new();
    voltra_scene::format::load(path, &ComponentRegistry::with_defaults(), &mut world)?;
    Ok(world)
}

/// Logs what the loaded scene holds, once, at startup.
///
/// The camera line is the useful half: a scene with none still draws, through
/// the default framing, and saying so next to the entity count is what turns
/// "my game looks wrong" into "my camera is not enabled".
pub fn describe(world: &World, path: &Path) {
    let entities = world.query::<voltra_scene::SceneId>().count();
    match camera::active(world) {
        Some(entity) => log::info!(
            "loaded {} ({entities} entities), rendering through camera {}",
            path.display(),
            entity.index()
        ),
        None => log::info!(
            "loaded {} ({entities} entities), with no active camera",
            path.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use voltra_scene::{Collider, RigidBody};

    /// The sample this repository ships, resolved from the crate rather than
    /// from the working directory, which `cargo test` does not promise.
    fn sandbox() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenes/sandbox.ron")
    }

    #[test]
    fn the_shipped_sandbox_scene_still_loads() {
        let world = load(&sandbox()).expect("the sample scene parses");

        assert!(
            camera::active(&world).is_some(),
            "the sample is what a build is run against: it has to frame itself"
        );
        assert!(
            world.query::<RigidBody>().count() >= 2,
            "and it has to move, or it demonstrates nothing about a running game"
        );
        assert!(world.query::<Collider>().count() >= 2);
    }

    #[test]
    fn a_scene_that_is_not_there_is_an_error_rather_than_an_empty_world() {
        let missing = Path::new(env!("CARGO_MANIFEST_DIR")).join("no-such-scene.ron");
        assert!(load(&missing).is_err());
    }
}
