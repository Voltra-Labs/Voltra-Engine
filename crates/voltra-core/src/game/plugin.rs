//! The two symbols a game library exports, written by
//! [`export_game!`](crate::export_game), and the logger handed across.
//!
//! - `voltra_game_stamp`, a C function answering a [`Stamp`]: how the
//!   library was built. Read first, and the only call made into a library
//!   that fails it.
//! - `voltra_game_new`, a Rust function answering a fresh `Box<dyn Game>`:
//!   called once the stamp matches, which is what makes the Rust types on
//!   both sides the same types.
//!
//! A library carries its own copy of every crate it links, `log` included,
//! and its copy has no logger: without one a game's `log::info!` would go
//! nowhere. `voltra_game_new` is handed the host's as one C function per
//! record, so the library forwards rather than sharing a Rust trait object
//! across two copies of the crate that defines it.

use std::sync::OnceLock;

use super::stamp::Stamp;
use crate::Game;

/// The symbol answering the library's [`Stamp`].
pub const STAMP_SYMBOL: &[u8] = b"voltra_game_stamp\0";
/// The symbol making a new game.
pub const NEW_SYMBOL: &[u8] = b"voltra_game_new\0";

/// `voltra_game_stamp`'s type.
pub type StampFn = extern "C" fn() -> Stamp;
/// `voltra_game_new`'s type. Rust's own calling convention: it is called
/// only once the stamps match, and hands back a Rust value.
pub type NewFn = fn(HostLog) -> Box<dyn Game>;

/// The host's logger, as a library sees it.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HostLog {
    /// Logs one record: its level (`log::Level as u32`, 1 = error to
    /// 5 = trace), its target and its message, each as UTF-8 bytes.
    pub log: extern "C" fn(
        level: u32,
        target: *const u8,
        target_len: usize,
        message: *const u8,
        message_len: usize,
    ),
    /// The most verbose level the host keeps (`log::LevelFilter as u32`).
    pub max_level: u32,
}

impl HostLog {
    /// This process's logger, for a library it loads.
    pub fn of_this_process() -> Self {
        Self {
            log: log_in_host,
            max_level: log::max_level() as u32,
        }
    }
}

/// Logs a record a library forwarded, through this process's logger.
extern "C" fn log_in_host(
    level: u32,
    target: *const u8,
    target_len: usize,
    message: *const u8,
    message_len: usize,
) {
    let text = |bytes: *const u8, len: usize| {
        if bytes.is_null() || len == 0 {
            return String::new();
        }
        // SAFETY: the library passes a pointer and a length taken from one
        // `&str` that outlives the call.
        let slice = unsafe { std::slice::from_raw_parts(bytes, len) };
        String::from_utf8_lossy(slice).into_owned()
    };
    let level = level_of(level);
    let target = text(target, target_len);
    let message = text(message, message_len);
    log::logger().log(
        &log::Record::builder()
            .level(level)
            .target(&target)
            .args(format_args!("{message}"))
            .build(),
    );
}

fn level_of(level: u32) -> log::Level {
    match level {
        1 => log::Level::Error,
        2 => log::Level::Warn,
        3 => log::Level::Info,
        4 => log::Level::Debug,
        _ => log::Level::Trace,
    }
}

fn filter_of(level: u32) -> log::LevelFilter {
    match level {
        0 => log::LevelFilter::Off,
        1 => log::LevelFilter::Error,
        2 => log::LevelFilter::Warn,
        3 => log::LevelFilter::Info,
        4 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    }
}

/// A library's logger: every record handed to the host.
struct Forward(HostLog);

impl log::Log for Forward {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= filter_of(self.0.max_level)
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let target = record.target();
        let message = record.args().to_string();
        (self.0.log)(
            record.level() as u32,
            target.as_ptr(),
            target.len(),
            message.as_ptr(),
            message.len(),
        );
    }

    fn flush(&self) {}
}

static FORWARD: OnceLock<Forward> = OnceLock::new();

/// Sends this library's log to the host. What `voltra_game_new` calls
/// first; a second call keeps the first logger, since `log` takes one.
#[doc(hidden)]
pub fn install_host_log(host: HostLog) {
    let forward = FORWARD.get_or_init(|| Forward(host));
    if log::set_logger(forward).is_ok() {
        log::set_max_level(filter_of(host.max_level));
    }
}

/// Makes the crate it is written in a game library the editor can load:
/// exports the stamp and a constructor making the game from `$make`, an
/// expression evaluated for every play session.
///
/// ```ignore
/// voltra_core::export_game!(Platformer::default());
/// ```
///
/// The crate builds as a `cdylib` for the editor beside its usual `rlib`;
/// its own binary links the `rlib` and passes the game to
/// [`App::with_game`](crate::App::with_game).
#[macro_export]
macro_rules! export_game {
    ($make:expr) => {
        /// How this library was built, for the host to check first.
        #[no_mangle]
        pub extern "C" fn voltra_game_stamp() -> $crate::game::stamp::Stamp {
            $crate::game::stamp::Stamp::current()
        }

        /// A fresh game, once the host has checked the stamp.
        #[no_mangle]
        #[allow(clippy::no_mangle_with_rust_abi)]
        pub fn voltra_game_new(
            host: $crate::game::plugin::HostLog,
        ) -> ::std::boxed::Box<dyn $crate::Game> {
            $crate::game::plugin::install_host_log(host);
            ::std::boxed::Box::new($make)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_survive_the_crossing() {
        for level in [
            log::Level::Error,
            log::Level::Warn,
            log::Level::Info,
            log::Level::Debug,
            log::Level::Trace,
        ] {
            assert_eq!(level_of(level as u32), level);
        }
        assert_eq!(
            filter_of(log::LevelFilter::Off as u32),
            log::LevelFilter::Off
        );
        assert_eq!(
            filter_of(log::LevelFilter::Info as u32),
            log::LevelFilter::Info
        );
    }

    static SEEN: std::sync::Mutex<Vec<(u32, String, String)>> = std::sync::Mutex::new(Vec::new());

    extern "C" fn record(
        level: u32,
        target: *const u8,
        target_len: usize,
        message: *const u8,
        message_len: usize,
    ) {
        // SAFETY: as in `log_in_host`.
        let text = |p, n| unsafe {
            String::from_utf8_lossy(std::slice::from_raw_parts(p, n)).into_owned()
        };
        SEEN.lock().expect("not poisoned").push((
            level,
            text(target, target_len),
            text(message, message_len),
        ));
    }

    #[test]
    fn a_record_crosses_as_its_level_target_and_text_and_below_the_filter_does_not() {
        let forward = Forward(HostLog {
            log: record,
            max_level: log::LevelFilter::Info as u32,
        });
        for (level, message) in [
            (log::Level::Warn, "coin taken"),
            (log::Level::Debug, "too fine"),
        ] {
            log::Log::log(
                &forward,
                &log::Record::builder()
                    .level(level)
                    .target("platformer")
                    .args(format_args!("{message}"))
                    .build(),
            );
        }
        let seen = SEEN.lock().expect("not poisoned");
        assert_eq!(
            *seen,
            vec![(2, "platformer".to_owned(), "coin taken".to_owned())]
        );
    }
}
