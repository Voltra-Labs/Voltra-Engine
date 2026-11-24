//! A window that is not there, for tests: it does what it is asked, as a
//! desktop would, and writes down every call.

use super::display::{Fullscreen, Monitor, VideoMode};
use super::seam::{Abilities, Shown, WindowSeam};

/// One call the loop made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Call {
    Fullscreen(Fullscreen),
    Size(u32, u32),
    Vsync(bool),
}

/// A desktop with one 1920 × 1080 monitor that also does 1280 × 720 —
/// or, with [`FakeWindow::browser`], a page.
#[derive(Debug, Clone)]
pub(crate) struct FakeWindow {
    pub(crate) shown: Shown,
    pub(crate) abilities: Abilities,
    pub(crate) monitors: Vec<Monitor>,
    pub(crate) calls: Vec<Call>,
}

pub(crate) const FULL_HD: VideoMode = VideoMode {
    width: 1920,
    height: 1080,
    refresh_millihertz: 60_000,
    bit_depth: 32,
};

pub(crate) const HD: VideoMode = VideoMode {
    width: 1280,
    height: 720,
    refresh_millihertz: 60_000,
    bit_depth: 32,
};

impl FakeWindow {
    pub(crate) fn desktop() -> Self {
        Self {
            shown: Shown {
                fullscreen: Fullscreen::Windowed,
                size: (1280, 720),
                vsync: true,
            },
            abilities: Abilities::default(),
            monitors: vec![Monitor {
                name: Some("fake".to_owned()),
                width: 1920,
                height: 1080,
                modes: vec![FULL_HD, HD],
                current: true,
            }],
            calls: Vec::new(),
        }
    }

    /// No size to set, vsync fixed, no modes: what a page allows.
    pub(crate) fn browser() -> Self {
        let mut page = Self::desktop();
        page.abilities = Abilities {
            resize: false,
            vsync: false,
        };
        page.monitors.clear();
        page
    }
}

impl WindowSeam for FakeWindow {
    fn abilities(&self) -> Abilities {
        self.abilities
    }

    fn shown(&self) -> Shown {
        self.shown
    }

    fn monitors(&self) -> Vec<Monitor> {
        self.monitors.clone()
    }

    fn set_fullscreen(&mut self, mode: Fullscreen) -> Fullscreen {
        let has_mode = |wanted: &VideoMode| {
            self.monitors
                .iter()
                .any(|monitor| monitor.modes.contains(wanted))
        };
        let granted = match mode {
            Fullscreen::Exclusive(wanted) if !has_mode(&wanted) => Fullscreen::Borderless,
            other => other,
        };
        self.calls.push(Call::Fullscreen(granted));
        self.shown.fullscreen = granted;
        self.shown.size = match granted {
            Fullscreen::Windowed => self.shown.size,
            Fullscreen::Borderless => (1920, 1080),
            Fullscreen::Exclusive(mode) => mode.size(),
        };
        granted
    }

    fn set_size(&mut self, width: u32, height: u32) {
        self.calls.push(Call::Size(width, height));
        self.shown.size = (width, height);
    }

    fn set_vsync(&mut self, on: bool) {
        self.calls.push(Call::Vsync(on));
        self.shown.vsync = on;
    }
}
