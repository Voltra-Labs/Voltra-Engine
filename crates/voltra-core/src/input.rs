//! Keyboard and mouse state.
//!
//! `winit` reports discrete press and release *events*; game code wants to ask
//! "is this key down right now?" and "was it pressed this frame?". This module
//! bridges the two.
//!
//! The winit decoding ([`Input::process_window_event`]) is deliberately a thin
//! shell over backend-agnostic mutators like [`Input::on_key`]. `winit`'s
//! `KeyEvent` carries a private platform-specific field and cannot be
//! constructed outside the crate, so a design that folded events directly
//! would be untestable. Splitting them also leaves room for a second input
//! source later.

use std::collections::HashSet;

use voltra_render::glam::Vec2;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub use winit::event::MouseButton;

/// Snapshot of input state for the current frame.
///
/// Call [`Input::end_frame`] once per frame **after** the frame is handled;
/// nothing else clears the per-frame sets.
#[derive(Debug, Default, Clone)]
pub struct Input {
    keys_held: HashSet<KeyCode>,
    keys_pressed: HashSet<KeyCode>,
    keys_released: HashSet<KeyCode>,

    buttons_held: HashSet<MouseButton>,
    buttons_pressed: HashSet<MouseButton>,
    buttons_released: HashSet<MouseButton>,

    cursor: Vec2,
    /// `None` until the cursor is first seen, so the initial jump from the
    /// origin is not reported as a delta.
    last_cursor: Option<Vec2>,
    mouse_delta: Vec2,
    scroll_delta: f32,
}

impl Input {
    pub fn new() -> Self {
        Self::default()
    }

    // --- backend-agnostic mutators -----------------------------------------

    /// Folds a key transition into the state.
    ///
    /// `repeat` events are ignored for the "pressed this frame" set: holding a
    /// key must report a press exactly once.
    pub fn on_key(&mut self, code: KeyCode, pressed: bool, repeat: bool) {
        if pressed {
            if !repeat {
                self.keys_pressed.insert(code);
            }
            self.keys_held.insert(code);
        } else {
            self.keys_held.remove(&code);
            self.keys_released.insert(code);
        }
    }

    pub fn on_mouse_button(&mut self, button: MouseButton, pressed: bool) {
        if pressed {
            self.buttons_pressed.insert(button);
            self.buttons_held.insert(button);
        } else {
            self.buttons_held.remove(&button);
            self.buttons_released.insert(button);
        }
    }

    pub fn on_cursor_moved(&mut self, position: Vec2) {
        self.cursor = position;
        if let Some(last) = self.last_cursor {
            self.mouse_delta += position - last;
        }
        self.last_cursor = Some(position);
    }

    pub fn on_scroll(&mut self, lines: f32) {
        self.scroll_delta += lines;
    }

    /// Drops every held key and button.
    ///
    /// Without this, alt-tabbing while holding a key leaves it stuck down
    /// forever: the release lands on whatever window took focus. winit only
    /// synthesises releases on X11 and Windows, so this is the portable fix.
    pub fn on_focus_lost(&mut self) {
        for code in self.keys_held.drain() {
            self.keys_released.insert(code);
        }
        for button in self.buttons_held.drain() {
            self.buttons_released.insert(button);
        }
    }

    // --- winit decoding -----------------------------------------------------

    /// Folds one winit event into the state. Unhandled events are ignored.
    pub fn process_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.on_key(code, event.state == ElementState::Pressed, event.repeat);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.on_mouse_button(*button, *state == ElementState::Pressed);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.on_cursor_moved(Vec2::new(position.x as f32, position.y as f32));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    // Pixel deltas come from trackpads and are far larger per
                    // event; scale them into roughly line-sized units.
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 120.0,
                };
                self.on_scroll(lines);
            }
            WindowEvent::Focused(false) => self.on_focus_lost(),
            _ => {}
        }
    }

    /// Clears the per-frame sets. Held state survives.
    pub fn end_frame(&mut self) {
        self.keys_pressed.clear();
        self.keys_released.clear();
        self.buttons_pressed.clear();
        self.buttons_released.clear();
        self.mouse_delta = Vec2::ZERO;
        self.scroll_delta = 0.0;
    }

    // --- queries ------------------------------------------------------------

    pub fn is_key_down(&self, code: KeyCode) -> bool {
        self.keys_held.contains(&code)
    }

    pub fn was_key_pressed(&self, code: KeyCode) -> bool {
        self.keys_pressed.contains(&code)
    }

    pub fn was_key_released(&self, code: KeyCode) -> bool {
        self.keys_released.contains(&code)
    }

    pub fn is_mouse_button_down(&self, button: MouseButton) -> bool {
        self.buttons_held.contains(&button)
    }

    pub fn was_mouse_button_pressed(&self, button: MouseButton) -> bool {
        self.buttons_pressed.contains(&button)
    }

    pub fn was_mouse_button_released(&self, button: MouseButton) -> bool {
        self.buttons_released.contains(&button)
    }

    /// Cursor position in physical pixels, origin at the window's top-left.
    pub fn cursor_position(&self) -> Vec2 {
        self.cursor
    }

    /// Cursor movement accumulated since the last [`Input::end_frame`].
    pub fn mouse_delta(&self) -> Vec2 {
        self.mouse_delta
    }

    /// Scroll accumulated since the last [`Input::end_frame`], in lines.
    pub fn scroll_delta(&self) -> f32 {
        self.scroll_delta
    }

    /// `+1` while `positive` is held, `-1` while `negative` is held, `0` when
    /// neither or both are. Saves every caller writing the same branch.
    pub fn axis(&self, negative: KeyCode, positive: KeyCode) -> f32 {
        f32::from(self.is_key_down(positive)) - f32::from(self.is_key_down(negative))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: KeyCode = KeyCode::KeyW;
    const S: KeyCode = KeyCode::KeyS;

    #[test]
    fn press_registers_as_held_and_pressed() {
        let mut input = Input::new();
        input.on_key(W, true, false);

        assert!(input.is_key_down(W));
        assert!(input.was_key_pressed(W));
        assert!(!input.was_key_released(W));
    }

    #[test]
    fn end_frame_clears_pressed_but_keeps_held() {
        let mut input = Input::new();
        input.on_key(W, true, false);
        input.end_frame();

        assert!(input.is_key_down(W), "the key is still physically down");
        assert!(!input.was_key_pressed(W), "but it is no longer a new press");
    }

    #[test]
    fn release_clears_held_and_reports_released() {
        let mut input = Input::new();
        input.on_key(W, true, false);
        input.end_frame();
        input.on_key(W, false, false);

        assert!(!input.is_key_down(W));
        assert!(input.was_key_released(W));
    }

    #[test]
    fn auto_repeat_does_not_re_trigger_a_press() {
        let mut input = Input::new();
        input.on_key(W, true, false);
        input.end_frame();

        // The OS keeps sending presses while the key is held.
        input.on_key(W, true, true);

        assert!(input.is_key_down(W), "still held");
        assert!(
            !input.was_key_pressed(W),
            "a repeat must not read as a fresh press"
        );
    }

    #[test]
    fn losing_focus_releases_everything_held() {
        let mut input = Input::new();
        input.on_key(W, true, false);
        input.on_mouse_button(MouseButton::Left, true);
        input.end_frame();

        input.on_focus_lost();

        assert!(!input.is_key_down(W), "a held key must not survive alt-tab");
        assert!(!input.is_mouse_button_down(MouseButton::Left));
        assert!(input.was_key_released(W));
        assert!(input.was_mouse_button_released(MouseButton::Left));
    }

    #[test]
    fn first_cursor_event_reports_no_delta() {
        let mut input = Input::new();
        input.on_cursor_moved(Vec2::new(400.0, 300.0));

        assert_eq!(input.cursor_position(), Vec2::new(400.0, 300.0));
        assert_eq!(
            input.mouse_delta(),
            Vec2::ZERO,
            "the jump from the origin is not movement"
        );
    }

    #[test]
    fn cursor_deltas_accumulate_within_a_frame() {
        let mut input = Input::new();
        input.on_cursor_moved(Vec2::new(100.0, 100.0));
        input.on_cursor_moved(Vec2::new(110.0, 100.0));
        input.on_cursor_moved(Vec2::new(110.0, 130.0));

        assert_eq!(input.mouse_delta(), Vec2::new(10.0, 30.0));

        input.end_frame();
        assert_eq!(input.mouse_delta(), Vec2::ZERO);
        assert_eq!(
            input.cursor_position(),
            Vec2::new(110.0, 130.0),
            "position is absolute and survives the frame boundary"
        );
    }

    #[test]
    fn scroll_accumulates_and_resets() {
        let mut input = Input::new();
        input.on_scroll(1.0);
        input.on_scroll(-0.5);

        assert_eq!(input.scroll_delta(), 0.5);
        input.end_frame();
        assert_eq!(input.scroll_delta(), 0.0);
    }

    #[test]
    fn axis_cancels_when_both_are_held() {
        let mut input = Input::new();
        assert_eq!(input.axis(S, W), 0.0);

        input.on_key(W, true, false);
        assert_eq!(input.axis(S, W), 1.0);

        input.on_key(S, true, false);
        assert_eq!(input.axis(S, W), 0.0, "opposing keys cancel");

        input.on_key(W, false, false);
        assert_eq!(input.axis(S, W), -1.0);
    }
}
