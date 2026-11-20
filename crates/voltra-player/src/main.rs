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

// A shipped build is double-clicked, and a console window behind the game is
// not part of it. Only with `windowed`, which `cargo xtask dist` turns on, so
// `cargo run -p voltra-player` keeps printing to the terminal it ran in.
#![cfg_attr(all(windows, feature = "windowed"), windows_subsystem = "windows")]

mod args;
mod launch;
mod scene;

use std::path::PathBuf;
use std::process::ExitCode;

use args::Parsed;
use voltra_core::{App, WindowConfig};

fn main() -> ExitCode {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from));
    init_log(exe_dir.as_deref());

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

    let launch = match launch::resolve(&args, exe_dir.as_deref()) {
        Ok(launch) => launch,
        Err(error) => {
            log::error!("{error}");
            println!("{}", args::USAGE);
            return ExitCode::FAILURE;
        }
    };

    // Before the window: a build that cannot read its scene has nothing to
    // show, and a window opening onto an empty world reads as a working build
    // of a broken game.
    let world = match scene::load(&launch.scene) {
        Ok(world) => world,
        Err(error) => {
            log::error!("could not load {}: {error}", launch.scene.display());
            return ExitCode::FAILURE;
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
    let mut app = app.with_simulation();
    if let Some(root) = launch.asset_root {
        app = app.with_asset_root(root);
    }

    // Runs until the window closes. Anything after this line is unreachable
    // until `App::run` learns to return, so the exit code is the one above.
    app.run();
    ExitCode::SUCCESS
}

/// The log goes to the terminal — or, in a windowed build that has none, to
/// `log.txt` beside the executable, so a build that fails on a double-click
/// says why somewhere a player can find it.
fn init_log(exe_dir: Option<&std::path::Path>) {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    if cfg!(feature = "windowed") {
        if let Some(file) = exe_dir.and_then(|dir| std::fs::File::create(dir.join("log.txt")).ok())
        {
            builder.target(env_logger::Target::Pipe(Box::new(file)));
        }
    }
    builder.init();
}
