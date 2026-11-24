//! The real window behind `tick.window`: winit's, and the renderer's
//! surface for vsync — carried out between frames.
//!
//! The one place the game's window requests meet the platform. Natively
//! winit does what is asked; exclusive fullscreen needs a mode of the
//! window's monitor that matches, else it is borderless. In a browser the
//! canvas goes fullscreen through the Fullscreen API — winit 0.30 calls
//! `requestFullscreen` straight away, which the browser grants only while
//! the page has transient activation from a key, click or touch; a tick
//! reacting to one is carried out within that window of a few seconds —
//! and the size and vsync are the page's and the browser's.

use voltra_render::Renderer;
use winit::window::Window;

use super::App;
use crate::window::seam::{Abilities, Shown, WindowSeam};
use crate::window::{Fullscreen, GameWindow, Monitor, VideoMode};

/// The window and the renderer drawing into it, borrowed for one call.
pub(super) struct WinitWindow<'a> {
    pub(super) window: &'a Window,
    pub(super) renderer: Option<&'a mut Renderer>,
}

impl WindowSeam for WinitWindow<'_> {
    fn abilities(&self) -> Abilities {
        let native = !cfg!(target_arch = "wasm32");
        Abilities {
            resize: native,
            vsync: native && self.renderer.is_some(),
        }
    }

    fn shown(&self) -> Shown {
        let size = self.window.inner_size();
        let fullscreen = match self.window.fullscreen() {
            None => Fullscreen::Windowed,
            Some(winit::window::Fullscreen::Borderless(_)) => Fullscreen::Borderless,
            Some(winit::window::Fullscreen::Exclusive(mode)) => {
                Fullscreen::Exclusive(video_mode(&mode))
            }
        };
        Shown {
            fullscreen,
            size: (size.width, size.height),
            vsync: self.renderer.as_ref().is_none_or(|r| r.vsync()),
        }
    }

    fn monitors(&self) -> Vec<Monitor> {
        // A page has no monitors to list (winit's web backend asks the
        // browser for a permission to), and no modes to switch to.
        if cfg!(target_arch = "wasm32") {
            return Vec::new();
        }
        let current = self.window.current_monitor();
        self.window
            .available_monitors()
            .map(|monitor| {
                let size = monitor.size();
                Monitor {
                    name: monitor.name(),
                    width: size.width,
                    height: size.height,
                    modes: monitor.video_modes().map(|m| video_mode(&m)).collect(),
                    current: current.as_ref() == Some(&monitor),
                }
            })
            .collect()
    }

    fn set_fullscreen(&mut self, mode: Fullscreen) -> Fullscreen {
        use winit::window::Fullscreen as Winit;
        let (asked, granted) = match mode {
            Fullscreen::Windowed => (None, mode),
            Fullscreen::Borderless => (Some(Winit::Borderless(None)), mode),
            Fullscreen::Exclusive(wanted) => match self.find_mode(wanted) {
                Some(handle) => (Some(Winit::Exclusive(handle)), mode),
                None => (Some(Winit::Borderless(None)), Fullscreen::Borderless),
            },
        };
        self.window.set_fullscreen(asked);
        granted
    }

    fn set_size(&mut self, width: u32, height: u32) {
        // `None` is "asked, the resize event will say": nothing to do now.
        let _ = self
            .window
            .request_inner_size(winit::dpi::PhysicalSize::new(width, height));
    }

    fn set_vsync(&mut self, on: bool) {
        if let Some(renderer) = self.renderer.as_deref_mut() {
            renderer.set_vsync(on);
        }
    }
}

impl WinitWindow<'_> {
    /// The window's monitor's mode matching `wanted`, else any monitor's;
    /// an exact refresh rate first, the size alone after.
    fn find_mode(&self, wanted: VideoMode) -> Option<winit::monitor::VideoModeHandle> {
        let monitors = self
            .window
            .current_monitor()
            .into_iter()
            .chain(self.window.available_monitors());
        let modes: Vec<_> = monitors.flat_map(|m| m.video_modes()).collect();
        modes
            .iter()
            .find(|mode| video_mode(mode) == wanted)
            .or_else(|| {
                modes
                    .iter()
                    .filter(|mode| video_mode(mode).size() == wanted.size())
                    .max_by_key(|mode| mode.refresh_rate_millihertz())
            })
            .cloned()
    }
}

fn video_mode(mode: &winit::monitor::VideoModeHandle) -> VideoMode {
    let size = mode.size();
    VideoMode {
        width: size.width,
        height: size.height,
        refresh_millihertz: mode.refresh_rate_millihertz(),
        bit_depth: mode.bit_depth(),
    }
}

impl App {
    /// The game window as the platform has it: after the window is made,
    /// and after a resize, which is also how a browser says fullscreen was
    /// granted.
    pub(super) fn refresh_game_window(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let seam = WinitWindow {
            window: &window,
            renderer: self.renderer.as_mut(),
        };
        self.game_window.refresh(&seam);
    }

    /// How the window is shown, read before the frame's ticks: cheap — the
    /// window's own state — and the only way to see a change the platform
    /// made with no event for it.
    pub(super) fn refresh_game_window_shown(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let seam = WinitWindow {
            window: &window,
            renderer: self.renderer.as_mut(),
        };
        self.game_window.refresh_shown(&seam);
    }

    /// Carries out the window requests the frame's ticks made, on the real
    /// window. Between frames: after the present, before the next is asked
    /// for.
    pub(super) fn apply_game_window(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let mut seam = WinitWindow {
            window: &window,
            renderer: self.renderer.as_mut(),
        };
        carry_out(&mut self.game_window, self.ui.is_some(), &mut seam);
    }

    /// What the frame's ticks asked of the window, done through `seam` —
    /// or, in the editor, whose window it is not, dropped.
    pub(super) fn carry_out_window_requests(&mut self, seam: &mut dyn WindowSeam) {
        carry_out(&mut self.game_window, self.ui.is_some(), seam);
    }
}

fn carry_out(game_window: &mut GameWindow, editor: bool, seam: &mut dyn WindowSeam) {
    if editor {
        game_window.drop_requests();
    } else {
        game_window.apply(seam);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::window::fake::{Call, FakeWindow, HD};

    const FRAME: f32 = 1.0 / 60.0;

    #[test]
    fn each_request_from_a_tick_reaches_the_window_between_frames() {
        // What the tick saw of the window each frame.
        let seen = Rc::new(RefCell::new(Vec::new()));
        let log = seen.clone();
        let mut frame = 0;
        let mut app = App::default().with_simulation().with_update(move |tick| {
            log.borrow_mut()
                .push((tick.window.fullscreen(), tick.window.size()));
            match frame {
                0 => {
                    assert_eq!(tick.window.modes().len(), 2, "the monitor's modes");
                    tick.window.set_fullscreen(Fullscreen::Exclusive(HD));
                }
                1 => tick.window.set_fullscreen(Fullscreen::Borderless),
                2 => {
                    tick.window.set_size(1024, 576);
                    tick.window.set_fullscreen(Fullscreen::Windowed);
                    tick.window.set_vsync(false);
                }
                _ => {}
            }
            frame += 1;
        });
        let mut window = FakeWindow::desktop();
        app.game_window.refresh(&window);
        for _ in 0..4 {
            app.advance(FRAME);
            app.carry_out_window_requests(&mut window);
        }

        assert_eq!(
            window.calls,
            vec![
                Call::Fullscreen(Fullscreen::Exclusive(HD)),
                Call::Fullscreen(Fullscreen::Borderless),
                Call::Fullscreen(Fullscreen::Windowed),
                Call::Size(1024, 576),
                Call::Vsync(false),
            ]
        );
        assert_eq!(
            *seen.borrow(),
            vec![
                (Fullscreen::Windowed, (1280, 720)),
                (Fullscreen::Exclusive(HD), (1280, 720)),
                (Fullscreen::Borderless, (1920, 1080)),
                (Fullscreen::Windowed, (1024, 576)),
            ],
            "each frame reads what the last one asked for"
        );
        assert!(!app.game_window.vsync());
    }

    #[test]
    fn the_editor_s_window_is_not_the_game_s() {
        let mut app = App::default()
            .with_ui(|_, _| {})
            .with_simulation()
            .with_update(|tick| tick.window.set_fullscreen(Fullscreen::Borderless));
        let mut window = FakeWindow::desktop();
        app.advance(FRAME);
        app.carry_out_window_requests(&mut window);
        assert!(window.calls.is_empty());
    }
}
