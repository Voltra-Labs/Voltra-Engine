//! A button that stays on or off.

use super::button::ButtonFrames;

/// Turns the [`UiButton`](super::UiButton) on the same entity into a
/// checkbox: each click flips `on`, and the change is reported as one.
///
/// Godot's `CheckBox` is a `Button` in toggle mode, and so is this — the
/// button keeps the hover, press and focus it already has, and its colours.
/// While on, the button's colour is multiplied by `on_tint`, and with
/// `on_frames` its frames are those instead of the button's own: a skin
/// with a checked and an unchecked face.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UiToggle {
    #[serde(default)]
    /// Whether it is on.
    pub on: bool,
    /// Multiplied into the button's colour while on.
    #[serde(default = "green")]
    pub on_tint: [f32; 4],
    /// The frames shown while on, if the faces are images.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_frames: Option<ButtonFrames>,
}

fn green() -> [f32; 4] {
    [0.45, 1.0, 0.55, 1.0]
}

impl Default for UiToggle {
    fn default() -> Self {
        Self {
            on: false,
            on_tint: green(),
            on_frames: None,
        }
    }
}

impl UiToggle {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "UiToggle";

    /// `color` as it shows in this state.
    pub fn tinted(&self, color: [f32; 4]) -> [f32; 4] {
        if !self.on {
            return color;
        }
        std::array::from_fn(|i| color[i] * self.on_tint[i])
    }
}
