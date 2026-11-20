//! Where a game's log goes.

use std::path::PathBuf;

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
/// Call once, first thing in `main`.
pub fn init(to_file: bool) {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    if to_file {
        if let Some(file) = log_path().and_then(|path| std::fs::File::create(path).ok()) {
            builder.target(env_logger::Target::Pipe(Box::new(file)));
        }
    }
    // A second call — a test harness, a game that also initialises — keeps
    // the first logger rather than panicking.
    let _ = builder.try_init();
}

/// [`LOG_FILE`] beside the running executable, if it can be found.
pub fn log_path() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(LOG_FILE)))
}
