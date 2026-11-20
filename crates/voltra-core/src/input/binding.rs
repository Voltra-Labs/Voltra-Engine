//! One physical control an action can be bound to.

use winit::event::MouseButton;
use winit::keyboard::KeyCode;

/// A key, a mouse button, a gamepad button or one direction of a stick, as
/// an input map file names it: `Key(Space)`, `Mouse(Left)`, `Pad(South)`,
/// `PadAxis(LeftStickX, Negative)`.
///
/// Keys are *physical* — `KeyW` is the key where W is on a US layout, which
/// is A on an AZERTY one. That is what Unity, Godot and Unreal bind by
/// default too: WASD stays a cross under the left hand on any layout.
///
/// Gamepad controls are named by position, not by label: `South` is A on an
/// Xbox pad and Cross on a PlayStation one. Any connected pad drives them —
/// one player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Binding {
    Key(KeyCode),
    Mouse(MouseButton),
    Pad(PadButton),
    PadAxis(PadAxis, Direction),
}

/// A gamepad button, by where it sits on a standard pad.
///
/// Ours rather than `gilrs::Button`, so the file format does not change with
/// the backend — and because `gilrs` calls the bumpers `LeftTrigger` and the
/// triggers `LeftTrigger2`, which no input map should have to say.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum PadButton {
    South,
    East,
    West,
    North,
    LeftBumper,
    RightBumper,
    /// Analog on most pads: its value is the action's strength.
    LeftTrigger,
    RightTrigger,
    Select,
    Start,
    Mode,
    /// Clicking the stick in.
    LeftStick,
    RightStick,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

/// A stick axis, `-1` to `+1`. X is positive right, Y positive **up**, as
/// `gilrs` reports them and as the world's y runs.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum PadAxis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
}

/// Which half of an axis a binding reads: a stick pushed left drives
/// `PadAxis(LeftStickX, Negative)` and nothing bound to `Positive`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Direction {
    Positive,
    Negative,
}

impl Direction {
    /// How much of `value` lies in this direction: `0` to `1`.
    pub fn amount(self, value: f32) -> f32 {
        let signed = match self {
            Self::Positive => value,
            Self::Negative => -value,
        };
        if signed.is_finite() {
            signed.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}
