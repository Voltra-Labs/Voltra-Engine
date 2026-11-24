use super::*;
use crate::window::fake::{Call, FakeWindow, FULL_HD, HD};

fn opened(seam: &FakeWindow) -> GameWindow {
    let mut window = GameWindow::default();
    window.refresh(seam);
    window
}

#[test]
fn nothing_happens_until_the_frame_is_over() {
    let mut seam = FakeWindow::desktop();
    let mut window = opened(&seam);
    window.set_fullscreen(Fullscreen::Borderless);
    assert!(window.is_pending());
    assert_eq!(window.fullscreen(), Fullscreen::Windowed, "not yet");
    assert!(seam.calls.is_empty());

    window.apply(&mut seam);
    assert_eq!(seam.calls, vec![Call::Fullscreen(Fullscreen::Borderless)]);
    assert_eq!(window.fullscreen(), Fullscreen::Borderless);
    assert_eq!(window.size(), (1920, 1080));
    assert!(!window.is_pending());

    window.apply(&mut seam);
    assert_eq!(seam.calls.len(), 1, "a request is carried out once");
}

#[test]
fn exclusive_takes_a_mode_and_falls_back_to_borderless_without_one() {
    let mut seam = FakeWindow::desktop();
    let mut window = opened(&seam);
    assert_eq!(window.modes(), &[FULL_HD, HD]);
    window.set_fullscreen(Fullscreen::Exclusive(HD));
    window.apply(&mut seam);
    assert_eq!(window.fullscreen(), Fullscreen::Exclusive(HD));
    assert_eq!(window.size(), (1280, 720));

    let odd = VideoMode { width: 999, ..HD };
    window.set_fullscreen(Fullscreen::Exclusive(odd));
    window.apply(&mut seam);
    assert_eq!(window.fullscreen(), Fullscreen::Borderless);
}

#[test]
fn a_windowed_size_asked_while_fullscreen_waits_for_the_window() {
    let mut seam = FakeWindow::desktop();
    let mut window = opened(&seam);
    window.set_fullscreen(Fullscreen::Borderless);
    window.apply(&mut seam);

    window.set_size(1600, 900);
    window.apply(&mut seam);
    assert!(
        !seam.calls.contains(&Call::Size(1600, 900)),
        "a fullscreen window is not resized"
    );
    assert_eq!(window.windowed_size(), (1600, 900));

    window.set_fullscreen(Fullscreen::Windowed);
    window.apply(&mut seam);
    assert_eq!(
        &seam.calls[seam.calls.len() - 2..],
        &[
            Call::Fullscreen(Fullscreen::Windowed),
            Call::Size(1600, 900)
        ]
    );
    assert_eq!(window.size(), (1600, 900));
}

#[test]
fn a_size_and_vsync_reach_a_window() {
    let mut seam = FakeWindow::desktop();
    let mut window = opened(&seam);
    window.set_size(1024, 576);
    window.set_vsync(false);
    window.apply(&mut seam);
    assert_eq!(seam.calls, vec![Call::Size(1024, 576), Call::Vsync(false)]);
    assert_eq!(window.size(), (1024, 576));
    assert!(!window.vsync());
}

#[test]
fn a_zero_size_is_refused() {
    let mut window = GameWindow::default();
    window.set_size(0, 600);
    assert!(!window.is_pending());
}

#[test]
fn a_page_takes_fullscreen_and_nothing_else() {
    let mut seam = FakeWindow::browser();
    let mut window = opened(&seam);
    assert!(window.modes().is_empty());
    assert!(!window.abilities().resize && !window.abilities().vsync);

    window.set_size(800, 600);
    window.set_vsync(false);
    window.set_fullscreen(Fullscreen::Exclusive(HD));
    window.apply(&mut seam);
    assert_eq!(seam.calls, vec![Call::Fullscreen(Fullscreen::Borderless)]);
    assert!(window.vsync(), "the browser paces frames");
}

#[test]
fn the_editor_drops_what_the_game_asked() {
    let mut seam = FakeWindow::desktop();
    let mut window = opened(&seam);
    window.set_fullscreen(Fullscreen::Borderless);
    window.drop_requests();
    window.apply(&mut seam);
    assert!(seam.calls.is_empty());
}
