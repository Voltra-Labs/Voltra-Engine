//! Gamepad state, merged across every connected pad.

use std::collections::{HashMap, HashSet};

use super::binding::{Direction, PadAxis, PadButton};

/// How far a stick must move this frame, past half way, for
/// [`Input::pressed_binding`](super::Input::pressed_binding) to report it:
/// enough that resting drift never rebinds an action.
pub const AXIS_PRESS: f32 = 0.5;

/// Buttons and axes of every pad, by pad.
///
/// Kept per pad so one pad releasing a button another still holds does not
/// release it, and a disconnect drops exactly that pad's state. Read merged:
/// one player, any pad.
#[derive(Debug, Clone, Default)]
pub struct Pads {
    /// Every button each pad holds down, as the backend reports presses.
    held: HashSet<(u32, PadButton)>,
    /// Analog travel of every button above zero, `0` to `1` — a trigger can
    /// be part-way in without being held.
    values: HashMap<(u32, PadButton), f32>,
    pressed: HashSet<PadButton>,
    released: HashSet<PadButton>,
    axes: HashMap<(u32, PadAxis), f32>,
    /// `axes` as they were at the last `end_frame`, for crossing tests.
    axes_before: HashMap<(u32, PadAxis), f32>,
}

impl Pads {
    pub fn on_button(&mut self, pad: u32, button: PadButton, pressed: bool) {
        if pressed {
            if !self.is_down(button) {
                self.pressed.insert(button);
            }
            self.held.insert((pad, button));
        } else {
            self.held.remove(&(pad, button));
            self.values.remove(&(pad, button));
            if !self.is_down(button) {
                self.released.insert(button);
            }
        }
    }

    /// An analog button's travel, `0` to `1`. Does not press or release on
    /// its own: the backend reports those as their own events, at its own
    /// threshold.
    pub fn on_button_value(&mut self, pad: u32, button: PadButton, value: f32) {
        let value = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if value > 0.0 {
            self.values.insert((pad, button), value);
        } else {
            self.values.remove(&(pad, button));
        }
    }

    pub fn on_axis(&mut self, pad: u32, axis: PadAxis, value: f32) {
        let value = if value.is_finite() {
            value.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.axes.insert((pad, axis), value);
    }

    /// Drops a pad that went away — or every pad when the window lost
    /// focus — releasing what it held.
    pub fn release(&mut self, pad: Option<u32>) {
        let matches = |p: &u32| pad.is_none_or(|pad| pad == *p);
        let held: Vec<PadButton> = self
            .held
            .iter()
            .filter(|(p, _)| matches(p))
            .map(|(_, b)| *b)
            .collect();
        self.held.retain(|(p, _)| !matches(p));
        self.values.retain(|(p, _), _| !matches(p));
        for button in held {
            if !self.is_down(button) {
                self.released.insert(button);
            }
        }
        self.axes.retain(|(p, _), _| !matches(p));
    }

    pub fn end_frame(&mut self) {
        self.pressed.clear();
        self.released.clear();
        self.axes_before.clone_from(&self.axes);
    }

    /// Whether any pad holds `button`.
    pub fn is_down(&self, button: PadButton) -> bool {
        self.held.iter().any(|(_, b)| *b == button)
    }

    pub fn was_pressed(&self, button: PadButton) -> bool {
        self.pressed.contains(&button)
    }

    pub fn was_released(&self, button: PadButton) -> bool {
        self.released.contains(&button)
    }

    /// The furthest any pad has `button` in, `0` to `1`. A held button with
    /// no analog value — a face button — is fully in.
    pub fn button_value(&self, button: PadButton) -> f32 {
        let travel = self
            .values
            .iter()
            .filter(|((_, b), _)| *b == button)
            .map(|(_, v)| *v)
            .fold(0.0, f32::max);
        let pressed_flat = self
            .held
            .iter()
            .any(|key| key.1 == button && !self.values.contains_key(key));
        if pressed_flat {
            1.0
        } else {
            travel
        }
    }

    /// The axis of whichever pad pushes it furthest, `-1` to `+1`.
    pub fn axis(&self, axis: PadAxis) -> f32 {
        Self::furthest(&self.axes, axis)
    }

    fn furthest(axes: &HashMap<(u32, PadAxis), f32>, axis: PadAxis) -> f32 {
        axes.iter()
            .filter(|((_, a), _)| *a == axis)
            .map(|(_, v)| *v)
            .fold(
                0.0,
                |best: f32, v| if v.abs() > best.abs() { v } else { best },
            )
    }

    /// The lowest button pressed this frame.
    pub fn pressed_button(&self) -> Option<PadButton> {
        self.pressed.iter().min().copied()
    }

    /// A stick direction pushed past [`AXIS_PRESS`] since the last frame.
    pub fn pushed_axis(&self) -> Option<(PadAxis, Direction)> {
        let mut pushed: Vec<(PadAxis, Direction)> = Vec::new();
        for axis in [
            PadAxis::LeftStickX,
            PadAxis::LeftStickY,
            PadAxis::RightStickX,
            PadAxis::RightStickY,
        ] {
            let now = self.axis(axis);
            let before = Self::furthest(&self.axes_before, axis);
            for direction in [Direction::Positive, Direction::Negative] {
                if direction.amount(now) >= AXIS_PRESS && direction.amount(before) < AXIS_PRESS {
                    pushed.push((axis, direction));
                }
            }
        }
        pushed.into_iter().min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_button_held_on_two_pads_is_released_when_both_let_go() {
        let mut pads = Pads::default();
        pads.on_button(0, PadButton::South, true);
        pads.on_button(1, PadButton::South, true);
        pads.end_frame();
        pads.on_button(0, PadButton::South, false);
        assert!(pads.is_down(PadButton::South));
        assert!(!pads.was_released(PadButton::South));
        pads.on_button(1, PadButton::South, false);
        assert!(pads.was_released(PadButton::South));
    }

    #[test]
    fn the_second_pad_pressing_a_held_button_is_not_a_new_press() {
        let mut pads = Pads::default();
        pads.on_button(0, PadButton::South, true);
        pads.end_frame();
        pads.on_button(1, PadButton::South, true);
        assert!(!pads.was_pressed(PadButton::South));
    }

    #[test]
    fn an_axis_reads_the_furthest_pad() {
        let mut pads = Pads::default();
        pads.on_axis(0, PadAxis::LeftStickX, 0.3);
        pads.on_axis(1, PadAxis::LeftStickX, -0.8);
        assert_eq!(pads.axis(PadAxis::LeftStickX), -0.8);
    }

    #[test]
    fn a_trigger_value_is_its_travel() {
        let mut pads = Pads::default();
        pads.on_button_value(0, PadButton::RightTrigger, 0.4);
        assert_eq!(pads.button_value(PadButton::RightTrigger), 0.4);
        pads.on_button(0, PadButton::RightTrigger, true);
        assert_eq!(pads.button_value(PadButton::RightTrigger), 0.4, "kept");
        pads.on_button_value(0, PadButton::RightTrigger, 2.0);
        assert_eq!(pads.button_value(PadButton::RightTrigger), 1.0, "clamped");
    }

    #[test]
    fn a_trigger_part_way_in_is_not_held() {
        let mut pads = Pads::default();
        pads.on_button_value(0, PadButton::LeftTrigger, 0.05);
        assert!(!pads.is_down(PadButton::LeftTrigger));
        assert_eq!(pads.button_value(PadButton::LeftTrigger), 0.05);
        pads.on_button(0, PadButton::South, true);
        assert_eq!(
            pads.button_value(PadButton::South),
            1.0,
            "a face button is all in"
        );
    }

    #[test]
    fn a_disconnect_releases_only_that_pad() {
        let mut pads = Pads::default();
        pads.on_button(0, PadButton::East, true);
        pads.on_button(1, PadButton::North, true);
        pads.on_axis(0, PadAxis::LeftStickY, 1.0);
        pads.end_frame();
        pads.release(Some(0));
        assert!(pads.was_released(PadButton::East));
        assert!(pads.is_down(PadButton::North));
        assert_eq!(pads.axis(PadAxis::LeftStickY), 0.0);
    }

    #[test]
    fn a_stick_pushed_past_half_way_this_frame_is_reported_once() {
        let mut pads = Pads::default();
        pads.on_axis(0, PadAxis::LeftStickX, -0.9);
        assert_eq!(
            pads.pushed_axis(),
            Some((PadAxis::LeftStickX, Direction::Negative))
        );
        pads.end_frame();
        assert_eq!(pads.pushed_axis(), None, "held there is not pushed again");
    }

    #[test]
    fn drift_is_not_a_push() {
        let mut pads = Pads::default();
        pads.on_axis(0, PadAxis::RightStickY, 0.1);
        assert_eq!(pads.pushed_axis(), None);
    }

    #[test]
    fn nonsense_values_read_as_rest() {
        let mut pads = Pads::default();
        pads.on_axis(0, PadAxis::LeftStickX, f32::NAN);
        assert_eq!(pads.axis(PadAxis::LeftStickX), 0.0);
        assert_eq!(Direction::Positive.amount(f32::INFINITY), 0.0);
    }
}
