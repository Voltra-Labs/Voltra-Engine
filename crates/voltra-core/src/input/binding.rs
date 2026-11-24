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
    /// A physical key.
    Key(KeyCode),
    /// A mouse button.
    Mouse(MouseButton),
    /// A gamepad button.
    Pad(PadButton),
    /// One direction of a gamepad stick axis.
    PadAxis(PadAxis, Direction),
    /// A finger that went down in a part of the screen: held, or dragged
    /// one way — a virtual stick. `Touch(BottomLeft, Right)` walks right
    /// while a thumb pressed in the bottom-left corner is slid right;
    /// `Touch(BottomRight, Hold)` is an on-screen jump button.
    Touch(TouchZone, TouchDir),
}

/// The part of the screen a touch binding listens to: where a finger has
/// to go down to drive it. It may wander out after.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum TouchZone {
    /// Anywhere on the screen.
    Anywhere,
    /// The left half of the screen.
    LeftHalf,
    /// The right half of the screen.
    RightHalf,
    /// The left half of the bottom half: where a left thumb rests.
    BottomLeft,
    /// The right half of the bottom half: where a right thumb rests.
    BottomRight,
}

impl TouchZone {
    /// Whether `point`, in pixels from the top-left of a `screen`, is in it.
    pub fn contains(
        self,
        point: voltra_render::glam::Vec2,
        screen: voltra_render::glam::Vec2,
    ) -> bool {
        let (left, bottom) = (point.x < screen.x * 0.5, point.y >= screen.y * 0.5);
        match self {
            Self::Anywhere => true,
            Self::LeftHalf => left,
            Self::RightHalf => !left,
            Self::BottomLeft => left && bottom,
            Self::BottomRight => !left && bottom,
        }
    }

    /// Its middle, on a `screen`: where a simulated finger goes down.
    pub fn centre(self, screen: voltra_render::glam::Vec2) -> voltra_render::glam::Vec2 {
        let (x, y) = match self {
            Self::Anywhere => (0.5, 0.5),
            Self::LeftHalf => (0.25, 0.5),
            Self::RightHalf => (0.75, 0.5),
            Self::BottomLeft => (0.25, 0.75),
            Self::BottomRight => (0.75, 0.75),
        };
        screen * voltra_render::glam::Vec2::new(x, y)
    }
}

/// What a touch binding reads of its finger: held at all, or dragged one
/// way from where it went down.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum TouchDir {
    /// Held, wherever the finger is.
    Hold,
    /// Dragged left of where it went down.
    Left,
    /// Dragged right of where it went down.
    Right,
    /// Dragged up from where it went down.
    Up,
    /// Dragged down from where it went down.
    Down,
}

impl TouchDir {
    /// How far an `offset` from where the finger went down — screen
    /// pixels, y down — lies this way, as a share of `radius`: `0` to `1`.
    /// `Hold` is `1` for any finger.
    pub fn amount(self, offset: voltra_render::glam::Vec2, radius: f32) -> f32 {
        let along = match self {
            Self::Hold => return 1.0,
            Self::Left => -offset.x,
            Self::Right => offset.x,
            Self::Up => -offset.y,
            Self::Down => offset.y,
        };
        if radius > 0.0 {
            (along / radius).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
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
    /// The bottom face button: A on an Xbox pad, Cross on a PlayStation one.
    South,
    /// The right face button: B on an Xbox pad, Circle on a PlayStation one.
    East,
    /// The left face button: X on an Xbox pad, Square on a PlayStation one.
    West,
    /// The top face button: Y on an Xbox pad, Triangle on a PlayStation one.
    North,
    /// The left shoulder button.
    LeftBumper,
    /// The right shoulder button.
    RightBumper,
    /// Analog on most pads: its value is the action's strength.
    LeftTrigger,
    /// The right trigger; analog on most pads.
    RightTrigger,
    /// The back or select button.
    Select,
    /// The start or menu button.
    Start,
    /// The centre logo button: Guide, Home.
    Mode,
    /// Clicking the stick in.
    LeftStick,
    /// Clicking the right stick in.
    RightStick,
    /// D-pad up.
    DPadUp,
    /// D-pad down.
    DPadDown,
    /// D-pad left.
    DPadLeft,
    /// D-pad right.
    DPadRight,
}

/// A stick axis, `-1` to `+1`. X is positive right, Y positive **up**, as
/// `gilrs` reports them and as the world's y runs.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum PadAxis {
    /// The left stick, left to right.
    LeftStickX,
    /// The left stick, down to up.
    LeftStickY,
    /// The right stick, left to right.
    RightStickX,
    /// The right stick, down to up.
    RightStickY,
}

/// Which half of an axis a binding reads: a stick pushed left drives
/// `PadAxis(LeftStickX, Negative)` and nothing bound to `Positive`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Direction {
    /// The axis pushed towards +1.
    Positive,
    /// The axis pushed towards -1.
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
