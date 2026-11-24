//! `voltra-server`: a scene run headless at a fixed tick, hosted for
//! clients. See [`voltra_server`] for the library a game's own server
//! binary is built on, and [`voltra_server::cli`] for the command line it
//! shares with this one.

use std::process::ExitCode;

use voltra_core::{App, WindowConfig};

fn main() -> ExitCode {
    voltra_server::cli::main("voltra-server", |_| {
        App::new(WindowConfig {
            title: "voltra-server".into(),
            ..WindowConfig::default()
        })
    })
}
