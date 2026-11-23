//! The log in a browser: its console, by level.
//!
//! `env_logger` writes to stderr, which on `wasm32-unknown-unknown` goes
//! nowhere. Each level goes to the console method that files it under the
//! same name, so the browser's own level filter works on engine lines.

use log::{Level, LevelFilter, Log, Metadata, Record};
use wasm_bindgen::JsValue;
use web_sys::console;

/// What the web logs: `info` and up, as natively without `RUST_LOG` — a
/// page has no environment to set one in.
const LEVEL: LevelFilter = LevelFilter::Info;

/// The console logger, and a panic hook that sends a panic's message there
/// too — without one a panic in a wasm build is only "unreachable executed".
pub(super) fn platform_logger(_to_file: bool) -> (Box<dyn Log>, LevelFilter) {
    std::panic::set_hook(Box::new(|info| {
        console::error_1(&JsValue::from_str(&info.to_string()));
    }));
    (Box::new(Console), LEVEL)
}

struct Console;

impl Log for Console {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= LEVEL
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = JsValue::from_str(&format!("[{}] {}", record.target(), record.args()));
        match record.level() {
            Level::Error => console::error_1(&line),
            Level::Warn => console::warn_1(&line),
            Level::Info => console::info_1(&line),
            Level::Debug | Level::Trace => console::debug_1(&line),
        }
    }

    fn flush(&self) {}
}
