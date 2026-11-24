//! The display rows' choices: which resolutions to offer, and which display
//! mode comes after which.

use crate::window::{Fullscreen, GameWindow};

/// What the resolution row offers when the monitor lists no modes — headless,
/// some Wayland compositors: the common 16:9 sizes, as most games offer.
const COMMON: [(u32, u32); 6] = [
    (3840, 2160),
    (2560, 1440),
    (1920, 1080),
    (1600, 900),
    (1366, 768),
    (1280, 720),
];

/// The sizes the resolution row offers, smallest first — the slider's right
/// is bigger — the monitor's own
/// modes, else the common sizes that fit on it — and `current`, so the row
/// can show where it starts. None where the size is not the game's to set
/// (the web).
pub(super) fn resolutions(window: &GameWindow, current: (u32, u32)) -> Vec<(u32, u32)> {
    if !window.abilities().resize {
        return Vec::new();
    }
    let monitor = window.monitor();
    let mut sizes = monitor.map(|m| m.resolutions()).unwrap_or_default();
    if sizes.is_empty() {
        let fits = |&(w, h): &(u32, u32)| monitor.is_none_or(|m| w <= m.width && h <= m.height);
        sizes = COMMON.iter().copied().filter(fits).collect();
    }
    if current.0 > 0 && current.1 > 0 && !sizes.contains(&current) {
        sizes.push(current);
    }
    sizes.sort_by_key(|&(w, h)| (u64::from(w) * u64::from(h), w));
    sizes.dedup();
    sizes
}

/// The display modes the dropdown offers: windowed, fullscreen, and
/// exclusive at `size` where the monitor can switch to it.
pub(super) fn modes(window: &GameWindow, size: (u32, u32)) -> Vec<Fullscreen> {
    let exclusive = window.monitor().and_then(|m| {
        m.best_mode(size.0, size.1)
            .or_else(|| m.modes.first().copied())
    });
    let mut modes = vec![Fullscreen::Windowed, Fullscreen::Borderless];
    modes.extend(exclusive.map(Fullscreen::Exclusive));
    modes
}

/// Where `now` is among `modes`: an exclusive mode matches the exclusive
/// entry whatever its size, as the dropdown has one.
pub(super) fn index_of(modes: &[Fullscreen], now: Fullscreen) -> usize {
    modes
        .iter()
        .position(|m| std::mem::discriminant(m) == std::mem::discriminant(&now))
        .unwrap_or(0)
}

/// The exclusive mode at `size`, when the window is exclusive: a new size
/// picked is a new mode. `None` when it is not exclusive, or the monitor has
/// no mode that size.
pub(super) fn exclusive_at(
    window: &GameWindow,
    now: Fullscreen,
    size: (u32, u32),
) -> Option<Fullscreen> {
    if !matches!(now, Fullscreen::Exclusive(_)) {
        return None;
    }
    window
        .monitor()
        .and_then(|m| m.best_mode(size.0, size.1))
        .map(Fullscreen::Exclusive)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::fake::{FakeWindow, FULL_HD, HD};

    fn window(seam: &FakeWindow) -> GameWindow {
        let mut window = GameWindow::default();
        window.refresh(seam);
        window
    }

    #[test]
    fn a_monitor_s_modes_are_offered_else_the_common_sizes() {
        let desktop = window(&FakeWindow::desktop());
        assert_eq!(
            resolutions(&desktop, (1280, 720)),
            vec![(1280, 720), (1920, 1080)]
        );
        assert_eq!(
            resolutions(&desktop, (1000, 700)),
            vec![(1000, 700), (1280, 720), (1920, 1080)],
            "where it starts is offered too"
        );

        let mut bare = FakeWindow::desktop();
        bare.monitors[0].modes.clear();
        let sizes = resolutions(&window(&bare), (1280, 720));
        assert_eq!(
            sizes.last(),
            Some(&(1920, 1080)),
            "none larger than the monitor"
        );
        assert!(sizes.contains(&(1600, 900)));

        assert!(resolutions(&window(&FakeWindow::browser()), (960, 540)).is_empty());
    }

    #[test]
    fn the_display_modes_offered() {
        let desktop = window(&FakeWindow::desktop());
        let size = (1280, 720);
        let modes = modes(&desktop, size);
        assert_eq!(
            modes,
            vec![
                Fullscreen::Windowed,
                Fullscreen::Borderless,
                Fullscreen::Exclusive(HD),
            ]
        );
        assert_eq!(index_of(&modes, Fullscreen::Exclusive(FULL_HD)), 2);
        assert_eq!(index_of(&modes, Fullscreen::Borderless), 1);
        assert_eq!(
            exclusive_at(&desktop, Fullscreen::Exclusive(HD), (1920, 1080)),
            Some(Fullscreen::Exclusive(FULL_HD))
        );

        let page = window(&FakeWindow::browser());
        assert_eq!(
            super::modes(&page, size).len(),
            2,
            "no exclusive without modes"
        );
    }
}
