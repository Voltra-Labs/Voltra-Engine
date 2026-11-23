//! The platformer as a build: its first level, its game, a window.
//!
//! Run it with `cargo run -p voltra-platformer`, or package it with
//! `cargo xtask dist --manifest platformer.build.ron`. It also runs in a
//! browser, built for `wasm32-unknown-unknown` beside the pack `cargo xtask
//! pack --manifest platformer.build.ron` writes. In the editor the same
//! game is loaded from this crate's library instead.

// A packaged build is double-clicked: no console behind the game in
// release, and the log in `log.txt` beside it (`logging::init` below). Debug
// builds keep the terminal they were run from.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use voltra_assets::AssetPath;
use voltra_core::{App, WindowConfig};
use voltra_platformer::{Platformer, FIRST_LEVEL};

fn main() {
    voltra_core::logging::init(cfg!(not(debug_assertions)));
    // Natively at once; in a browser once the game's pack has arrived.
    voltra_core::when_assets_ready(start);
}

fn start() -> Result<(), String> {
    let level = AssetPath::new(FIRST_LEVEL).map_err(|e| e.to_string())?;
    let mut app = App::new(WindowConfig {
        title: "Voltra — platformer".into(),
        ..Default::default()
    })
    // Its saves folder: `%APPDATA%/Voltra Platformer/saves` on Windows.
    .with_game_name("Voltra Platformer")
    .with_game(Platformer::default());
    app.load_scene(&level);
    app.with_simulation().run();
    Ok(())
}
