//! From a resolved launch to an app ready to run, on every target.

use voltra_core::{App, WindowConfig};

use crate::launch::Launch;
use crate::scene;

/// Loads the scene and builds the app around it; `None`, logged, when the
/// scene cannot be read.
pub fn app(launch: Launch) -> Option<App> {
    // Before the window: a build that cannot read its scene has nothing to
    // show, and a window opening onto an empty world reads as a working build
    // of a broken game.
    let world = match scene::load(&launch.scene) {
        Ok(world) => world,
        Err(error) => {
            log::error!("could not load {}: {error}", launch.scene.display());
            return None;
        }
    };
    scene::describe(&world, &launch.scene);

    let defaults = WindowConfig::default();
    let (width, height) = launch.size.unwrap_or((defaults.width, defaults.height));
    let mut app = App::new(WindowConfig {
        title: launch.title.clone(),
        width,
        height,
    });
    app.world = world;

    // Physics from the first frame, and no hot reload: a game simulates the
    // moment it starts — there is no authoring state to protect — and it has
    // no reason to watch files it will never see change.
    // Saves go under the game's own name, which a build's manifest gives.
    let mut app = app.with_simulation().with_game_name(&launch.title);
    if let Some(root) = launch.asset_root {
        app = app.with_asset_root(root);
    }
    Some(app)
}
