//! Reads by binding and by action, over the raw state in [`Input`].

use super::action::BindingSource;
use super::{ActionMap, ActionState, Binding, Input};

impl Input {
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
