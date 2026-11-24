//! What the engine logged, kept for the Console window and the bridge.

use log::Level;
use voltra_core::logging::{capture, LogCapture, LogLine};

/// Lines kept: a long session's worth of warnings, not every debug line.
const KEPT: usize = 2000;

/// The lines logged since the editor started, read from the log ring each
/// frame, and the window's filters.
///
/// Unity's Console and Godot's Output: the log the terminal shows, inside
/// the editor, filtered by level — so a warning about a missing texture is
/// seen without a terminal open.
pub(crate) struct Console {
    capture: LogCapture,
    lines: Vec<LogLine>,
    /// The quietest level shown: `Warn` shows warnings and errors.
    pub(crate) level: Level,
    /// Shown only when its target or message holds this; empty for all.
    pub(crate) filter: String,
    /// Lines that fell out of the ring before they were read.
    missed: u64,
    pub(crate) open: bool,
}

impl Default for Console {
    fn default() -> Self {
        Self {
            capture: capture(),
            lines: Vec::new(),
            level: Level::Info,
            filter: String::new(),
            missed: 0,
            open: false,
        }
    }
}

impl Console {
    /// Takes what was logged since the last frame.
    pub(crate) fn read(&mut self) {
        self.lines.extend(self.capture.drain());
        self.missed = self.capture.missed();
        if self.lines.len() > KEPT {
            let extra = self.lines.len() - KEPT;
            self.lines.drain(..extra);
        }
    }

    /// The kept lines at `level` or louder whose target or message holds
    /// `filter`, oldest first.
    pub(crate) fn shown<'a>(
        &'a self,
        level: Level,
        filter: &'a str,
    ) -> impl Iterator<Item = &'a LogLine> + 'a {
        let filter = filter.to_lowercase();
        self.lines.iter().filter(move |line| {
            line.level <= level
                && (filter.is_empty()
                    || line.message.to_lowercase().contains(&filter)
                    || line.target.to_lowercase().contains(&filter))
        })
    }

    pub(crate) fn clear(&mut self) {
        self.lines.clear();
    }

    pub(crate) fn missed(&self) -> u64 {
        self.missed
    }

    /// How many kept lines are at each of error, warn and info.
    pub(crate) fn counts(&self) -> [usize; 3] {
        let at = |level| self.lines.iter().filter(|l| l.level == level).count();
        [at(Level::Error), at(Level::Warn), at(Level::Info)]
    }
}

/// The level a filter names: `error`, `warn`, `info`, `debug` or `trace`.
pub(crate) fn level_named(name: &str) -> Option<Level> {
    match name.to_ascii_lowercase().as_str() {
        "error" => Some(Level::Error),
        "warn" | "warning" => Some(Level::Warn),
        "info" => Some(Level::Info),
        "debug" => Some(Level::Debug),
        "trace" => Some(Level::Trace),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(level: Level, message: &str) -> LogLine {
        LogLine {
            level,
            target: "voltra_assets::textures".to_owned(),
            message: message.to_owned(),
        }
    }

    #[test]
    fn a_level_shows_itself_and_louder_and_text_narrows_it() {
        let console = Console {
            lines: vec![
                line(Level::Error, "cannot read hero.png"),
                line(Level::Warn, "missing texture coin.png"),
                line(Level::Info, "loaded 4 entities"),
                line(Level::Debug, "chunk rebuilt"),
            ],
            ..Console::default()
        };
        assert_eq!(console.shown(Level::Warn, "").count(), 2);
        assert_eq!(console.shown(Level::Trace, "").count(), 4);
        assert_eq!(console.shown(Level::Trace, "PNG").count(), 2, "any case");
        assert_eq!(
            console.shown(Level::Info, "textures").count(),
            3,
            "the target too"
        );
        assert_eq!(console.counts(), [1, 1, 1]);
    }

    #[test]
    fn levels_are_named_as_the_log_names_them() {
        assert_eq!(level_named("Warning"), Some(Level::Warn));
        assert_eq!(level_named("nope"), None);
    }
}
