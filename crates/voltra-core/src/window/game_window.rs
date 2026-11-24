//! The window as a game's tick sees it: what it is now, and the changes the
//! game asked for, carried out by the loop between frames.
//!
//! Requests, like a scene change, and for the same reason: the tick runs
//! inside the frame, and a window that went fullscreen under it would
//! resize the surface the frame is being drawn into. Godot's
//! `DisplayServer.window_set_mode` and Unity's `Screen.SetResolution` are
//! deferred to the end of the frame the same way.

use super::display::{Fullscreen, Monitor, VideoMode};
use super::seam::{Abilities, Shown, WindowSeam};

/// The changes asked for this frame. The last of each kind wins.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Pending {
    fullscreen: Option<Fullscreen>,
    size: Option<(u32, u32)>,
    vsync: Option<bool>,
}

/// A game's window: how it fills the screen, its size, vsync and the
/// monitors' modes — read now, changed from the next frame on.
///
/// `tick.window`. What is read is the window as the loop last saw it; what
/// is set waits for the end of the frame. In the editor the requests are
/// dropped — the editor's window is not the game's to resize, as Unity's
/// `Screen.SetResolution` does nothing in its editor — and headless they
/// are granted at once, as a window that took them would.
#[derive(Debug, Clone, Default)]
pub struct GameWindow {
    shown: Shown,
    abilities: Abilities,
    monitors: Vec<Monitor>,
    pending: Pending,
    /// The windowed size last asked for, kept while fullscreen and applied
    /// when the window is windowed again.
    windowed_size: Option<(u32, u32)>,
}

impl GameWindow {
    /// How the window fills the screen now.
    pub fn fullscreen(&self) -> Fullscreen {
        self.shown.fullscreen
    }

    /// The inner size now, in physical pixels.
    pub fn size(&self) -> (u32, u32) {
        self.shown.size
    }

    /// Whether frames wait for the display's refresh now.
    pub fn vsync(&self) -> bool {
        self.shown.vsync
    }

    /// What this platform lets a game change: no size and no vsync on the
    /// web, where the page lays the canvas out and the browser paces frames.
    pub fn abilities(&self) -> Abilities {
        self.abilities
    }

    /// Every monitor, the window's marked [`Monitor::current`]. Empty before
    /// the window exists, headless, and on the web.
    pub fn monitors(&self) -> &[Monitor] {
        &self.monitors
    }

    /// The monitor the window is on, else the first there is.
    pub fn monitor(&self) -> Option<&Monitor> {
        self.monitors
            .iter()
            .find(|monitor| monitor.current)
            .or_else(|| self.monitors.first())
    }

    /// The modes exclusive fullscreen can switch the window's monitor to.
    pub fn modes(&self) -> &[VideoMode] {
        self.monitor()
            .map_or(&[], |monitor| monitor.modes.as_slice())
    }

    /// The windowed size the window takes when it is next windowed: the last
    /// asked for, else its size now.
    pub fn windowed_size(&self) -> (u32, u32) {
        self.windowed_size.unwrap_or(self.shown.size)
    }

    /// Asks for `mode` from the next frame on. In a browser it takes effect
    /// only after a key, click or touch (the Fullscreen API needs a
    /// gesture, which a tick reacting to one has had); exclusive is
    /// borderless where there is no mode to switch to.
    pub fn set_fullscreen(&mut self, mode: Fullscreen) {
        self.pending.fullscreen = Some(mode);
    }

    /// Asks for a windowed inner size, in physical pixels. While fullscreen
    /// it is kept for when the window is windowed again. Nothing on the web;
    /// a size of zero is not a window and is refused.
    pub fn set_size(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            log::warn!("a {width} × {height} window was asked for; keeping the size it has");
            return;
        }
        self.pending.size = Some((width, height));
    }

    /// Asks for vsync on or off: the surface's present mode. Nothing on the
    /// web, where the browser paces frames.
    pub fn set_vsync(&mut self, on: bool) {
        self.pending.vsync = Some(on);
    }

    /// Whether a change waits for the end of the frame.
    pub fn is_pending(&self) -> bool {
        self.pending != Pending::default()
    }

    /// Drops what was asked: the editor's way of keeping its window.
    pub(crate) fn drop_requests(&mut self) {
        if self.is_pending() {
            log::info!("the editor keeps its own window; the game's window requests are dropped");
        }
        self.pending = Pending::default();
    }

    /// Reads how the window is shown — fullscreen, size, vsync — without
    /// the monitors: once a frame, since a platform can change it with no
    /// event to say so (a browser granting fullscreen to a canvas already
    /// the size of the page; Escape leaving it).
    pub(crate) fn refresh_shown(&mut self, seam: &dyn WindowSeam) {
        self.shown = seam.shown();
    }

    /// Reads the window afresh: after it was made, moved or resized.
    pub(crate) fn refresh(&mut self, seam: &dyn WindowSeam) {
        self.abilities = seam.abilities();
        self.shown = seam.shown();
        self.monitors = seam.monitors();
    }

    /// Carries out what was asked, then reads the window back. Fullscreen
    /// first, so a windowed size is asked of a window that is windowed.
    pub(crate) fn apply(&mut self, seam: &mut dyn WindowSeam) {
        let pending = std::mem::take(&mut self.pending);
        if pending == Pending::default() {
            return;
        }
        let abilities = seam.abilities();
        if let Some(size) = pending.size {
            if abilities.resize {
                self.windowed_size = Some(size);
            } else {
                log::info!("the page lays the canvas out; a window size is not the game's to set");
            }
        }
        let target = pending.fullscreen.unwrap_or(self.shown.fullscreen);
        if let Some(mode) = pending.fullscreen {
            let granted = seam.set_fullscreen(mode);
            if granted != mode {
                log::warn!("{mode:?} is not available here; asked for {granted:?} instead");
            }
        }
        // A size waits for a window: asked for now if the window is (or is
        // becoming) windowed, else kept for when it is.
        let leaving = pending.fullscreen == Some(Fullscreen::Windowed);
        if abilities.resize && target == Fullscreen::Windowed && (pending.size.is_some() || leaving)
        {
            if let Some((width, height)) = self.windowed_size {
                seam.set_size(width, height);
            }
        }
        if let Some(on) = pending.vsync {
            if abilities.vsync {
                seam.set_vsync(on);
            } else {
                log::info!("the browser paces frames; vsync stays on");
            }
        }
        self.refresh(seam);
    }
}

#[cfg(test)]
mod tests;
