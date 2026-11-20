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

    /// Whether `binding` is down: a key or button held, a stick direction
    /// past [`AXIS_PRESS`](super::pads::AXIS_PRESS). Actions apply their own
    /// dead zone instead; this is the raw read.
    pub fn is_binding_down(&self, binding: Binding) -> bool {
        match binding {
            Binding::Key(code) => self.is_key_down(code),
            Binding::Mouse(button) => self.is_mouse_button_down(button),
            Binding::Pad(button) => self.pads.is_down(button),
            Binding::PadAxis(..) => self.binding_value(binding) >= super::pads::AXIS_PRESS,
        }
    }

    /// Whether `binding` was pressed this frame. A stick has no press of its
    /// own; see [`Input::pressed_binding`] for one being pushed.
    pub fn was_binding_pressed(&self, binding: Binding) -> bool {
        match binding {
            Binding::Key(code) => self.was_key_pressed(code),
            Binding::Mouse(button) => self.was_mouse_button_pressed(button),
            Binding::Pad(button) => self.pads.was_pressed(button),
            Binding::PadAxis(..) => false,
        }
    }

    /// How far `binding` is in, `0` to `1`: a key is `0` or `1`, a trigger
    /// its travel, a stick direction how far it is pushed that way.
    pub fn binding_value(&self, binding: Binding) -> f32 {
        match binding {
            Binding::Key(_) | Binding::Mouse(_) => {
                f32::from(u8::from(self.is_binding_down(binding)))
            }
            Binding::Pad(button) => self.pads.button_value(button),
            Binding::PadAxis(axis, direction) => direction.amount(self.pads.axis(axis)),
        }
    }

    /// A control that went down this frame, for "press a key for jump".
    ///
    /// Keys, then mouse buttons, then pad buttons, then a stick pushed past
    /// half way — the lowest of each when several went at once, so the
    /// answer does not depend on hash order.
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
            .or_else(|| self.pads.pressed_button().map(Binding::Pad))
            .or_else(|| {
                self.pads
                    .pushed_axis()
                    .map(|(axis, direction)| Binding::PadAxis(axis, direction))
            })
    }
}

impl BindingSource for Input {
    fn value(&self, binding: Binding) -> f32 {
        self.binding_value(binding)
    }

    fn went_down(&self, binding: Binding) -> bool {
        self.was_binding_pressed(binding)
    }
}
