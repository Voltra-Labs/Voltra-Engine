//! The window a headless session pretends to have: it grants every request
//! as a desktop window would, so a game that asks for fullscreen and reads
//! it back runs headless as it does shipped. No monitors, so exclusive is
//! borderless.

use crate::window::seam::{Abilities, Shown, WindowSeam};
use crate::window::{Fullscreen, Monitor};

/// A window that is only its state.
#[derive(Debug, Clone, Copy)]
pub(super) struct PretendWindow {
    shown: Shown,
}

impl PretendWindow {
    /// A window of `size` pixels, windowed, with vsync on.
    pub(super) fn new(size: (u32, u32)) -> Self {
        Self {
            shown: Shown {
                size,
                ..Shown::default()
            },
        }
    }
}

impl WindowSeam for PretendWindow {
    fn abilities(&self) -> Abilities {
        Abilities::default()
    }

    fn shown(&self) -> Shown {
        self.shown
    }

    fn monitors(&self) -> Vec<Monitor> {
        Vec::new()
    }

    fn set_fullscreen(&mut self, mode: Fullscreen) -> Fullscreen {
        let granted = match mode {
            Fullscreen::Exclusive(_) => Fullscreen::Borderless,
            other => other,
        };
        self.shown.fullscreen = granted;
        granted
    }

    fn set_size(&mut self, width: u32, height: u32) {
        self.shown.size = (width, height);
    }

    fn set_vsync(&mut self, on: bool) {
        self.shown.vsync = on;
    }
}
