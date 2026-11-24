//! Dodge as a build: its first scene, its game, a window.
//!
//! `cargo run -p dodge` runs it; `cargo xtask dist --manifest
//! games/dodge/build.ron` packages it, and `--web` for a browser.

// A packaged build is double-clicked: no console behind the game in
// release, and the log in `log.txt` beside it. Debug builds keep the
// terminal they were run from.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use dodge::{Dodge, FIRST_SCENE};
use voltra_assets::AssetPath;
use voltra_core::{App, WindowConfig};

fn main() {
    voltra_core::logging::init(cfg!(not(debug_assertions)));
    // Natively at once; in a browser once the game's pack has arrived.
    voltra_core::when_assets_ready(start);
}

fn start() -> Result<(), String> {
    let scene = AssetPath::new(FIRST_SCENE).map_err(|e| e.to_string())?;
    let mut app = App::new(WindowConfig {
        title: "Dodge".into(),
        ..Default::default()
    })
    .with_game_name("Dodge")
    .with_game(Dodge::default());
    app.load_scene(&scene);
    app.with_simulation().run();
    Ok(())
}
