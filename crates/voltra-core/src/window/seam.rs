//! What the loop needs of a real window to carry out a game's window
//! requests: one trait, so the requests can be driven against a fake in a
//! test, as the window's would be — and so the platform's own calls stay in
//! the one place that implements it (`app/window_seam.rs`).

use super::display::{Fullscreen, Monitor};

/// What a window can change on this platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Abilities {
    /// Whether its size can be asked for: not on the web, where the page
    /// lays the canvas out.
    pub resize: bool,
    /// Whether vsync can be turned off: not on the web, where the browser
    /// paces frames.
    pub vsync: bool,
}

impl Default for Abilities {
    fn default() -> Self {
        Self {
            resize: true,
            vsync: true,
        }
    }
}

/// The window as it is now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Shown {
    /// How it fills the screen.
    pub(crate) fullscreen: Fullscreen,
    /// Its inner size, in physical pixels.
    pub(crate) size: (u32, u32),
    /// Whether frames wait for the display's refresh.
    pub(crate) vsync: bool,
}

impl Default for Shown {
    fn default() -> Self {
        Self {
            fullscreen: Fullscreen::Windowed,
            size: (0, 0),
            vsync: true,
        }
    }
}

/// A window the loop can ask things of. Each call is made between frames.
pub(crate) trait WindowSeam {
    /// What it can change.
    fn abilities(&self) -> Abilities;

    /// How it is now. A request a platform grants later (the web's
    /// fullscreen, after its promise) shows here once granted.
    fn shown(&self) -> Shown;

    /// The monitors there are, the window's marked.
    fn monitors(&self) -> Vec<Monitor>;

    /// Asks for `mode`. Answers what was asked of the platform in its
    /// place: exclusive with no matching mode, or none at all, is
    /// borderless.
    fn set_fullscreen(&mut self, mode: Fullscreen) -> Fullscreen;

    /// Asks for an inner size, in physical pixels. Only called when
    /// [`Abilities::resize`] says it can.
    fn set_size(&mut self, width: u32, height: u32);

    /// Turns vsync on or off. Only called when [`Abilities::vsync`] says it
    /// can.
    fn set_vsync(&mut self, on: bool);
}
