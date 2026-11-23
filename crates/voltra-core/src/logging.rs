//! Where a game's log goes.
//!
//! To the terminal or a file through `env_logger` natively, to the browser's
//! console on the web — and, whatever else, into a ring of recent lines a
//! caller can read back ([`capture`]): what a headless run hands a tool as
//! "what the engine said while it ran".

mod capture;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

use std::path::PathBuf;

pub use capture::{capture, LogCapture, LogLine};

#[cfg(not(target_arch = "wasm32"))]
use native::platform_logger;
#[cfg(target_arch = "wasm32")]
use web::platform_logger;

/// The file a windowed build logs to, beside its executable.
pub const LOG_FILE: &str = "log.txt";

/// Starts the logger at `info` (or `RUST_LOG`), writing to the terminal — or,
/// with `to_file`, to [`LOG_FILE`] beside the executable.
///
/// `to_file` is for a build with no console: one double-clicked, compiled
/// with `windows_subsystem = "windows"`. Without it the log of a build that
/// fails on start goes nowhere, and the player sees a window that never
/// opens. A file that cannot be created falls back to the terminal.
///
/// On the web there is no terminal and no file: the log goes to the
/// browser's console, `to_file` or not, and so does a panic's message.
///
/// Call once, first thing in `main`.
pub fn init(to_file: bool) {
    let (logger, level) = platform_logger(to_file);
    // A second call — a test harness, a game that also initialises — keeps
    // the first logger rather than panicking.
    // Through the ring as well: `capture` reads what passed.
    if log::set_boxed_logger(Box::new(capture::Tee::new(logger))).is_ok() {
        log::set_max_level(level);
    }
}

/// [`LOG_FILE`] beside the running executable, if it can be found.
pub fn log_path() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(LOG_FILE)))
}
