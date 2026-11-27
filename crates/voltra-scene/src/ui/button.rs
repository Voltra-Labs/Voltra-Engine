//! Buttons: screen-space elements that take a click, a hover and the focus.

use voltra_ecs::{Entity, World};

use super::rect::UiRect;
use super::toggle::UiToggle;
use crate::sprite::Sprite2D;

/// Makes a [`UiRect`] element clickable and focusable, and tints its
/// `Sprite2D` by state — and, with [`frames`](Self::frames), swaps the atlas
/// frame it shows, so a button is drawn from an image per state.
///
/// Godot's `Button`, minus the text it carries: the label is a `Text`
/// child, and a click on the label reaches the button, as Godot's default
/// `mouse_filter` passes it. Colours rather than a style resource: a sprite
/// tint is what this engine's quads already have.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UiButton {
    /// The colour at rest, RGBA.
    pub normal: [f32; 4],
    /// Under the pointer, or holding the focus.
    pub hovered: [f32; 4],
    /// Held down.
    pub pressed: [f32; 4],
    /// The colour while disabled, RGBA.
    pub disabled_color: [f32; 4],
    /// Takes no click and no focus.
    #[serde(default)]
    pub disabled: bool,
    /// The frame of the sprite's atlas shown in each state, if the states
    /// are images; the tint above still applies, so an image button keeps
    /// its colours white. Not written when there are none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<ButtonFrames>,
}

/// One atlas frame per button state: Unity's sprite-swap transition.
///
/// Frames of the button's own `Sprite2D` atlas, rather than four images of
/// their own: a UI skin is one sheet, and the sprite already names it —
/// the frame is the one thing a state changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ButtonFrames {
    /// The frame at rest.
    pub normal: u32,
    /// The frame under the pointer, or holding the focus.
    pub hovered: u32,
    /// The frame while held down.
    pub pressed: u32,
    /// The frame while disabled.
    pub disabled: u32,
}

impl ButtonFrames {
    /// Four frames in a row from `first`: normal, hovered, pressed,
    /// disabled — how a skin sheet is usually cut.
    pub fn in_a_row(first: u32) -> Self {
        Self {
            normal: first,
            hovered: first + 1,
            pressed: first + 2,
            disabled: first + 3,
        }
    }
}

impl Default for UiButton {
    fn default() -> Self {
        Self {
            normal: [0.2, 0.22, 0.28, 1.0],
            hovered: [0.3, 0.34, 0.44, 1.0],
            pressed: [0.14, 0.15, 0.2, 1.0],
            disabled_color: [0.2, 0.2, 0.2, 0.5],
            disabled: false,
            frames: None,
        }
    }
}

/// What a button shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// At rest.
    Normal,
    /// Under the pointer, or holding the focus.
    Hovered,
    /// Held down.
    Pressed,
    /// Taking no click and no focus.
    Disabled,
}

impl UiButton {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "UiButton";

    /// Writes the face of `state` into `sprite`: the state's colour — times
    /// a toggle's tint while it is on — and, with frames, its frame.
    pub fn show(&self, state: State, toggle: Option<&UiToggle>, sprite: &mut Sprite2D) {
        let state = if self.disabled {
            State::Disabled
        } else {
            state
        };
        let color = match state {
            State::Normal => self.normal,
            State::Hovered => self.hovered,
            State::Pressed => self.pressed,
            State::Disabled => self.disabled_color,
        };
        sprite.color = toggle.map_or(color, |t| t.tinted(color));
        let frames = toggle
            .filter(|t| t.on)
            .and_then(|t| t.on_frames)
            .or(self.frames);
        if let Some(frames) = frames {
            sprite.frame = match state {
                State::Normal => frames.normal,
                State::Hovered => frames.hovered,
                State::Pressed => frames.pressed,
                State::Disabled => frames.disabled,
            };
        }
    }
}

/// Whether `entity` can be a button at all: it needs a rect to be clicked.
pub fn is_button(world: &World, entity: Entity) -> bool {
    world.get::<UiButton>(entity).is_some() && world.get::<UiRect>(entity).is_some()
}
