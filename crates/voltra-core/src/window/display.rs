//! What a screen can show: how the window fills it, the modes a monitor
//! offers, and the monitors there are — plain values a game can compare,
//! list and save, with nothing of the platform's in them.

/// How the window fills the screen.
///
/// Godot's `WindowMode` and Unity's `FullScreenMode`, cut to the three a
/// desktop has: a window, a borderless window over the whole monitor at its
/// own resolution, and exclusive fullscreen, which changes the monitor's
/// mode — where the platform allows it (not on Wayland or the web, where it
/// is borderless).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Fullscreen {
    /// A window with its frame.
    #[default]
    Windowed,
    /// The whole of the monitor the window is on, at the monitor's own mode.
    Borderless,
    /// The monitor switched to this mode, the window filling it.
    Exclusive(VideoMode),
}

impl Fullscreen {
    /// Whether the window covers the monitor, either way.
    pub fn is_fullscreen(self) -> bool {
        self != Self::Windowed
    }
}

/// One mode a monitor can be switched to for exclusive fullscreen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct VideoMode {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
    /// Refresh rate in millihertz: 59 940 for 59.94 Hz.
    pub refresh_millihertz: u32,
    /// Bits per pixel, usually 24 or 32.
    pub bit_depth: u16,
}

impl VideoMode {
    /// The refresh rate in hertz.
    pub fn refresh_hz(&self) -> f32 {
        self.refresh_millihertz as f32 / 1000.0
    }

    /// `width` × `height`, as a resolution list shows it.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// A monitor and the modes it offers.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Monitor {
    /// Its name, if the platform gives one.
    pub name: Option<String>,
    /// Its resolution now, in physical pixels.
    pub width: u32,
    /// Its resolution now, in physical pixels.
    pub height: u32,
    /// Every mode exclusive fullscreen can switch it to; empty where there
    /// is no exclusive fullscreen (the web).
    pub modes: Vec<VideoMode>,
    /// Whether the window is on this one.
    pub current: bool,
}

impl Monitor {
    /// The distinct resolutions of its modes, largest first: what an options
    /// screen lists, each once whatever refresh rates it comes at.
    pub fn resolutions(&self) -> Vec<(u32, u32)> {
        let mut sizes: Vec<(u32, u32)> = self.modes.iter().map(VideoMode::size).collect();
        sizes.sort_by_key(|&(w, h)| std::cmp::Reverse((u64::from(w) * u64::from(h), w)));
        sizes.dedup();
        sizes
    }

    /// The mode at `width` × `height` with the highest refresh rate, then
    /// the deepest colour: what a resolution picked from the list switches
    /// to.
    pub fn best_mode(&self, width: u32, height: u32) -> Option<VideoMode> {
        self.modes
            .iter()
            .filter(|mode| mode.size() == (width, height))
            .max_by_key(|mode| (mode.refresh_millihertz, mode.bit_depth))
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(width: u32, height: u32, hz: u32) -> VideoMode {
        VideoMode {
            width,
            height,
            refresh_millihertz: hz * 1000,
            bit_depth: 32,
        }
    }

    #[test]
    fn resolutions_are_listed_once_largest_first() {
        let monitor = Monitor {
            modes: vec![
                mode(1280, 720, 60),
                mode(1920, 1080, 60),
                mode(1920, 1080, 144),
                mode(800, 600, 60),
            ],
            ..Monitor::default()
        };
        assert_eq!(
            monitor.resolutions(),
            vec![(1920, 1080), (1280, 720), (800, 600)]
        );
        assert_eq!(monitor.best_mode(1920, 1080), Some(mode(1920, 1080, 144)));
        assert_eq!(monitor.best_mode(640, 480), None);
    }
}
