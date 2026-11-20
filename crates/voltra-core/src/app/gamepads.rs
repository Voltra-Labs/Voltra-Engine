//! Gamepads, through `gilrs`, folded into [`Input`] once per frame.
//!
//! The only file that names `gilrs`: it turns the backend's events into
//! [`Input`]'s backend-agnostic mutators, the way `process_window_event`
//! does for winit's, so everything past this point is testable without a
//! pad plugged in.

use gilrs::{Axis, Button, EventType, Gilrs};

use crate::input::{Input, PadAxis, PadButton};

/// The live connection to the platform's gamepad API.
pub(super) struct Gamepads {
    gilrs: Gilrs,
}

impl Gamepads {
    /// Connects, or `None` where there is no gamepad support — logged, and
    /// the game runs on keyboard and mouse.
    pub(super) fn connect() -> Option<Self> {
        match Gilrs::new() {
            Ok(gilrs) => {
                for (_, pad) in gilrs.gamepads() {
                    log::info!("gamepad connected: {}", pad.name());
                }
                Some(Self { gilrs })
            }
            Err(gilrs::Error::NotImplemented(_)) => {
                log::info!("no gamepad support on this platform");
                None
            }
            Err(e) => {
                log::warn!("gamepads unavailable: {e}");
                None
            }
        }
    }

    /// Drains every pending event into `input`. Never blocks.
    pub(super) fn poll(&mut self, input: &mut Input) {
        while let Some(event) = self.gilrs.next_event() {
            let pad = u32::try_from(usize::from(event.id)).unwrap_or(u32::MAX);
            match event.event {
                EventType::ButtonPressed(button, _) => {
                    if let Some(button) = button_of(button) {
                        input.on_pad_button(pad, button, true);
                    }
                }
                EventType::ButtonReleased(button, _) => {
                    if let Some(button) = button_of(button) {
                        input.on_pad_button(pad, button, false);
                    }
                }
                EventType::ButtonChanged(button, value, _) => {
                    if let Some(button) = button_of(button) {
                        input.on_pad_button_value(pad, button, value);
                    }
                }
                EventType::AxisChanged(axis, value, _) => {
                    if let Some(axis) = axis_of(axis) {
                        input.on_pad_axis(pad, axis, value);
                    }
                }
                EventType::Connected => {
                    log::info!("gamepad connected: {}", self.gilrs.gamepad(event.id).name());
                }
                EventType::Disconnected => {
                    log::info!(
                        "gamepad disconnected: {}",
                        self.gilrs.gamepad(event.id).name()
                    );
                    input.on_pad_disconnected(pad);
                }
                _ => {}
            }
        }
    }
}

/// `gilrs`'s button, by our name for its position. `gilrs` calls the
/// bumpers `LeftTrigger` and the analog triggers `LeftTrigger2`.
fn button_of(button: Button) -> Option<PadButton> {
    Some(match button {
        Button::South => PadButton::South,
        Button::East => PadButton::East,
        Button::West => PadButton::West,
        Button::North => PadButton::North,
        Button::LeftTrigger => PadButton::LeftBumper,
        Button::RightTrigger => PadButton::RightBumper,
        Button::LeftTrigger2 => PadButton::LeftTrigger,
        Button::RightTrigger2 => PadButton::RightTrigger,
        Button::Select => PadButton::Select,
        Button::Start => PadButton::Start,
        Button::Mode => PadButton::Mode,
        Button::LeftThumb => PadButton::LeftStick,
        Button::RightThumb => PadButton::RightStick,
        Button::DPadUp => PadButton::DPadUp,
        Button::DPadDown => PadButton::DPadDown,
        Button::DPadLeft => PadButton::DPadLeft,
        Button::DPadRight => PadButton::DPadRight,
        // C, Z and unknown: on no standard pad.
        _ => return None,
    })
}

/// `gilrs`'s stick axes. The `Z` axes and the D-pad axes are left out:
/// `gilrs` reports triggers and the D-pad as buttons on standard pads.
fn axis_of(axis: Axis) -> Option<PadAxis> {
    Some(match axis {
        Axis::LeftStickX => PadAxis::LeftStickX,
        Axis::LeftStickY => PadAxis::LeftStickY,
        Axis::RightStickX => PadAxis::RightStickX,
        Axis::RightStickY => PadAxis::RightStickY,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gilrs_bumpers_and_triggers_get_their_own_names() {
        assert_eq!(button_of(Button::LeftTrigger), Some(PadButton::LeftBumper));
        assert_eq!(
            button_of(Button::LeftTrigger2),
            Some(PadButton::LeftTrigger)
        );
        assert_eq!(button_of(Button::Unknown), None);
    }

    #[test]
    fn only_stick_axes_are_read() {
        assert_eq!(axis_of(Axis::RightStickY), Some(PadAxis::RightStickY));
        assert_eq!(axis_of(Axis::LeftZ), None);
    }
}
