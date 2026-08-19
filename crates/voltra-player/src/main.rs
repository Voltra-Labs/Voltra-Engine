//! The player: a scene, a window, and no editor.
//!
//! This is the binary a shipped game is. It is a separate crate rather than a
//! flag on `voltra-editor` for the reason every engine keeps the two apart —
//! Unity builds a Player, Godot exports against a template, Unreal ships
//! without the editor module: nothing an editor owns (egui, the panels, undo,
//! the gizmos) belongs in a build, and the surest way to keep it out is for
//! the build not to link it.
//!
//! What it does is the frame flow with the middle removed: load the scene,
//! open a window, simulate, and draw the world through the camera the scene
//! authored. Everything it needs already exists in the layers below; this file
//! is the wiring, and it stays that way.

mod args;
mod scene;

use std::process::ExitCode;

use args::Parsed;
use voltra_core::{App, WindowConfig};

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Skipping the executable's own path, which `parse` does not want to know
    // about — it is the one argument no user typed.
    let args = match args::parse(std::env::args().skip(1)) {
        Ok(Parsed::Run(args)) => args,
        Ok(Parsed::Help) => {
            println!("{}", args::USAGE);
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            log::error!("{error}");
            println!("{}", args::USAGE);
            return ExitCode::FAILURE;
        }
    };

    // Before the window: a build that cannot read its scene has nothing to
    // show, and a window opening onto an empty world reads as a working build
    // of a broken game.
    let world = match scene::load(&args.scene) {
        Ok(world) => world,
        Err(error) => {
            log::error!("could not load {}: {error}", args.scene.display());
            return ExitCode::FAILURE;
        }
    };
    scene::describe(&world, &args.scene);

    let defaults = WindowConfig::default();
    let (width, height) = args.size.unwrap_or((defaults.width, defaults.height));
    let mut app = App::new(WindowConfig {
        title: args::title(&args),
        width,
        height,
    });
    app.world = world;

    // Physics from the first frame, and no hot reload: a game simulates the
    // moment it starts — there is no authoring state to protect — and it has
    // no reason to watch files it will never see change.
    let mut app = app.with_simulation();
    if let Some(root) = args.asset_root {
        app = app.with_asset_root(root);
    }

    // Runs until the window closes. Anything after this line is unreachable
    // until `App::run` learns to return, so the exit code is the one above.
    app.run();
    ExitCode::SUCCESS
}
