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
//!
//! Named actions sit on top: an [`ActionMap`] says which controls drive
//! "jump", and [`Input::action`] answers for the action rather than the key.
//! The raw reads stay — an editor shortcut is a key, not an action.

pub mod action;
pub mod binding;
pub mod map;

use std::cell::RefCell;
use std::collections::HashSet;

pub use action::ActionState;
pub use binding::Binding;
pub use map::{Action, ActionMap, MapError};

use action::{ActionStates, BindingSource};

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

    /// Every action's state, re-derived by [`Input::update_actions`].
    actions: ActionStates,
    /// Action names asked for that the map does not have, so each is warned
    /// about once rather than every frame it is read.
    unknown: RefCell<HashSet<String>>,
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

    // --- actions ------------------------------------------------------------

    /// Re-derives every action of `map` from this frame's keys and buttons.
    ///
    /// Once per frame, after the frame's events and before anything reads an
    /// action; `App` does it at the top of each frame. A rebinding made
    /// during a frame is therefore seen from the next one.
    pub fn update_actions(&mut self, map: &ActionMap) {
        let mut actions = std::mem::take(&mut self.actions);
        actions.update(map, self);
        self.actions = actions;
    }

    /// What `name` is doing this frame.
    ///
    /// An action the map does not have is never held, and is logged once: a
    /// typo in an action name is the input equivalent of a missing texture,
    /// and should be as visible.
    pub fn action(&self, name: &str) -> ActionState {
        if let Some(state) = self.actions.get(name) {
            return state;
        }
        if self.unknown.borrow_mut().insert(name.to_owned()) {
            log::warn!("input action `{name}` is not in the input map; it will never fire");
        }
        ActionState::default()
    }

    /// [`Input::axis`] for two actions: `-1` to `+1` from `negative` and
    /// `positive`'s strengths.
    pub fn action_axis(&self, negative: &str, positive: &str) -> f32 {
        self.action(positive).strength() - self.action(negative).strength()
    }

    pub fn is_binding_down(&self, binding: Binding) -> bool {
        match binding {
            Binding::Key(code) => self.is_key_down(code),
            Binding::Mouse(button) => self.is_mouse_button_down(button),
        }
    }

    pub fn was_binding_pressed(&self, binding: Binding) -> bool {
        match binding {
            Binding::Key(code) => self.was_key_pressed(code),
            Binding::Mouse(button) => self.was_mouse_button_pressed(button),
        }
    }

    /// A control that went down this frame, for "press a key for jump".
    ///
    /// Keys before mouse buttons, and the lowest of each when several went
    /// down at once, so the answer does not depend on hash order.
    pub fn pressed_binding(&self) -> Option<Binding> {
        self.keys_pressed
            .iter()
            .min()
            .map(|code| Binding::Key(*code))
            .or_else(|| {
                self.buttons_pressed
                    .iter()
                    .min()
                    .map(|button| Binding::Mouse(*button))
            })
    }
}

impl BindingSource for Input {
    fn is_down(&self, binding: Binding) -> bool {
        self.is_binding_down(binding)
    }

    fn went_down(&self, binding: Binding) -> bool {
        self.was_binding_pressed(binding)
    }
}

#[cfg(test)]
mod tests;
