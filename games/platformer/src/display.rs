//! Fullscreen on F11 or the HUD's button: borderless, the window's monitor
//! at its own resolution, and back.

use voltra_core::{Binding, Fullscreen, KeyCode, Tick};

use crate::components::FullscreenButton;

/// Whether F11 went down, or the fullscreen button was clicked, this frame.
pub fn asked(tick: &Tick<'_>) -> bool {
    tick.input.was_binding_pressed(Binding::Key(KeyCode::F11))
        || tick
            .world
            .query::<FullscreenButton>()
            .any(|(button, _)| tick.ui.clicked(button))
}

/// Asks for fullscreen if the window is windowed, and the window back if
/// it is not — from the next frame on. In a browser the key or the click
/// is the gesture the Fullscreen API waits for.
pub fn toggle(tick: &mut Tick<'_>) {
    let next = if tick.window.fullscreen().is_fullscreen() {
        Fullscreen::Windowed
    } else {
        Fullscreen::Borderless
    };
    log::info!("asking for {next:?}");
    tick.window.set_fullscreen(next);
}
