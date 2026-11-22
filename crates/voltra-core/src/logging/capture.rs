//! A ring of the most recent log lines, and cursors that read what is new.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use log::{Level, Log, Metadata, Record};

/// How many lines the ring keeps: a few seconds of a chatty run. A reader
/// further behind than this has missed lines, and is told how many.
const KEPT: usize = 4096;

/// One line that was logged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub level: Level,
    /// The module that logged it.
    pub target: String,
    pub message: String,
}

struct Ring {
    lines: VecDeque<LogLine>,
    /// How many lines have ever been pushed: the sequence number of the
    /// next one.
    pushed: u64,
}

fn ring() -> &'static Mutex<Ring> {
    static RING: OnceLock<Mutex<Ring>> = OnceLock::new();
    RING.get_or_init(|| {
        Mutex::new(Ring {
            lines: VecDeque::with_capacity(KEPT),
            pushed: 0,
        })
    })
}

/// Reads the lines logged since it was made, or since it last read.
///
/// Every line the installed logger lets through goes in the ring, from
/// whatever thread; a capture sees all of them, not only its own thread's.
/// Nothing is captured unless [`super::init`] installed the logger — a game
/// that installed its own sees nothing here.
#[derive(Debug, Clone)]
pub struct LogCapture {
    next: u64,
    missed: u64,
}

/// A capture starting now.
pub fn capture() -> LogCapture {
    let next = ring().lock().map_or(0, |ring| ring.pushed);
    LogCapture { next, missed: 0 }
}

impl LogCapture {
    /// The lines since the last read, oldest first.
    pub fn drain(&mut self) -> Vec<LogLine> {
        let Ok(ring) = ring().lock() else {
            return Vec::new();
        };
        let oldest = ring.pushed - ring.lines.len() as u64;
        if self.next < oldest {
            self.missed += oldest - self.next;
            self.next = oldest;
        }
        let skip = (self.next - oldest) as usize;
        let lines: Vec<LogLine> = ring.lines.iter().skip(skip).cloned().collect();
        self.next = ring.pushed;
        lines
    }

    /// How many lines fell out of the ring before this capture read them.
    pub fn missed(&self) -> u64 {
        self.missed
    }
}

/// The installed logger: `env_logger`'s, with every line it lets through
/// copied into the ring.
pub(super) struct Tee {
    inner: env_logger::Logger,
}

impl Tee {
    pub(super) fn new(inner: env_logger::Logger) -> Self {
        Self { inner }
    }
}

impl Log for Tee {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.inner.enabled(metadata)
    }

    fn log(&self, record: &Record<'_>) {
        if !self.inner.matches(record) {
            return;
        }
        self.inner.log(record);
        push(record);
    }

    fn flush(&self) {
        self.inner.flush();
    }
}

fn push(record: &Record<'_>) {
    let Ok(mut ring) = ring().lock() else {
        return;
    };
    if ring.lines.len() == KEPT {
        ring.lines.pop_front();
    }
    ring.lines.push_back(LogLine {
        level: record.level(),
        target: record.target().to_owned(),
        message: record.args().to_string(),
    });
    ring.pushed += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capture_reads_each_line_once_and_says_what_fell_out() {
        crate::logging::init(false);
        let mut capture = capture();
        log::warn!("capture-test marker one");
        let lines = capture.drain();
        assert!(lines
            .iter()
            .any(|l| l.message == "capture-test marker one" && l.level == Level::Warn));
        log::warn!("capture-test marker two");
        let again = capture.drain();
        assert!(
            !again.iter().any(|l| l.message == "capture-test marker one"),
            "read once"
        );
        assert!(again.iter().any(|l| l.message == "capture-test marker two"));

        let mut behind = super::capture();
        for i in 0..KEPT + 10 {
            log::info!("capture-test flood {i}");
        }
        let flooded = behind.drain();
        assert!(flooded.len() <= KEPT);
        assert!(behind.missed() >= 10, "{}", behind.missed());
    }
}
