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
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod scene;
mod start;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    native::main()
}

#[cfg(target_arch = "wasm32")]
fn main() {
    web::main();
}
