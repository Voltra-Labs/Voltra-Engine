//! A UI theme: the skins, fonts, colours and spacing controls take when
//! they keep none of their own, from a `.theme.ron` file.

mod apply;

use std::fmt;
use std::path::Path;

pub use apply::apply;

use super::edges::Edges;
use super::slice::UiSlice;

/// The format a theme file is in.
pub const THEME_VERSION: u32 = 1;

/// `ui/<name>.theme.ron`: how every control looks unless it is marked
/// [`UiOwnStyle`]. Every section is optional — a theme that sets only the
/// buttons leaves the rest as authored — and the file starts with
/// `#![enable(implicit_some)]`, so a section is written without `Some(…)`:
///
/// ```ron
/// #![enable(implicit_some)]
/// (
///     version: 1,
///     font: "fonts/Ubuntu-Light.ttf",
///     text_color: (0.95, 0.95, 1.0, 1.0),
///     button: (
///         normal: (0.2, 0.22, 0.28, 1.0),
///         hovered: (0.3, 0.34, 0.44, 1.0),
///         pressed: (0.14, 0.15, 0.2, 1.0),
///         disabled: (0.2, 0.2, 0.2, 0.5),
///         skin: (texture: "ui/button.png", borders: (left: 6, top: 6, right: 6, bottom: 6)),
///     ),
///     spacing: 8.0,
/// )
/// ```
///
/// Godot's `Theme` resource and Unity's USS do the same job: one file a
/// game swaps for a look. Ours is applied once to each element when it
/// first appears and again to all when the theme changes
/// ([`UiControls::set_theme`](super::UiControls::set_theme)), writing into
/// the components what a scene would otherwise say — so code that colours
/// a label afterwards (a clash in the controls list) is not undone every
/// frame.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UiTheme {
    /// The format version, at most [`THEME_VERSION`].
    pub version: u32,
    /// The face of every UI text, by asset path.
    #[serde(default)]
    pub font: Option<String>,
    /// The colour of every UI text.
    #[serde(default)]
    pub text_color: Option<[f32; 4]>,
    /// Buttons — toggles and tabs too — and a dropdown's box.
    #[serde(default)]
    pub button: Option<ButtonStyle>,
    /// What a toggle's colour is multiplied by while it is on, and a tab's
    /// while its page shows.
    #[serde(default)]
    pub selected_tint: Option<[f32; 4]>,
    /// Panels: an element stacking others with a picture of its own.
    #[serde(default)]
    pub panel: Option<BoxStyle>,
    /// Text fields' boxes and selections.
    #[serde(default)]
    pub field: Option<FieldStyle>,
    /// Sliders' tracks, fills and handles.
    #[serde(default)]
    pub slider: Option<SliderStyle>,
    /// Dropdowns' lists.
    #[serde(default)]
    pub dropdown: Option<ListStyle>,
    /// Tooltips' boxes and text.
    #[serde(default)]
    pub tooltip: Option<TooltipStyle>,
    /// Points between the children of every stack.
    #[serde(default)]
    pub spacing: Option<f32>,
    /// Points inside the edges of every stack.
    #[serde(default)]
    pub padding: Option<f32>,
}

/// A 9-slice picture: a texture and its fixed borders.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Skin {
    /// The image, by asset path.
    pub texture: String,
    /// Its fixed border, in texels.
    pub borders: Edges,
    /// Points per texel.
    #[serde(default = "one")]
    pub scale: f32,
}

fn one() -> f32 {
    1.0
}

impl Skin {
    /// The slice the element is drawn as.
    pub fn slice(&self) -> UiSlice {
        UiSlice {
            borders: self.borders,
            scale: self.scale,
            center: true,
        }
    }
}

/// A button's colours by state, and its skin.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ButtonStyle {
    /// At rest.
    pub normal: [f32; 4],
    /// Under the pointer or holding the focus.
    pub hovered: [f32; 4],
    /// Held down.
    pub pressed: [f32; 4],
    /// Disabled.
    pub disabled: [f32; 4],
    /// The picture its colours tint.
    #[serde(default)]
    pub skin: Option<Skin>,
}

/// A box's colour and skin.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BoxStyle {
    /// Its colour, tinting the skin if there is one.
    pub color: [f32; 4],
    /// Its picture.
    #[serde(default)]
    pub skin: Option<Skin>,
}

/// A text field's look.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FieldStyle {
    /// The box.
    pub color: [f32; 4],
    /// Behind selected text.
    pub selection: [f32; 4],
    /// The box's picture.
    #[serde(default)]
    pub skin: Option<Skin>,
}

/// A slider's look.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SliderStyle {
    /// The track: the slider's own picture.
    pub track: [f32; 4],
    /// The fill up to the value.
    pub fill: [f32; 4],
    /// The handle.
    pub handle: [f32; 4],
}

/// A dropdown list's look.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ListStyle {
    /// Behind the items.
    pub background: [f32; 4],
    /// Behind the highlighted item.
    pub highlight: [f32; 4],
}

/// A tooltip's look.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TooltipStyle {
    /// Behind the text.
    pub background: [f32; 4],
    /// The text.
    pub color: [f32; 4],
    /// The text's em height, in points.
    #[serde(default)]
    pub size: Option<f32>,
}

/// Keeps a theme off this element: it looks as its scene says, whatever
/// theme is on. Godot's per-control theme overrides, all at once.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UiOwnStyle;

/// Why a theme did not read.
#[derive(Debug)]
pub enum ThemeError {
    /// The file could not be read.
    Io(std::io::Error),
    /// The file is not valid RON for a theme.
    Parse(ron::error::SpannedError),
    /// Written by a newer engine.
    Version(u32),
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "cannot read it: {e}"),
            Self::Parse(e) => write!(f, "cannot parse it: {e}"),
            Self::Version(v) => write!(f, "version {v} is newer than {THEME_VERSION}"),
        }
    }
}

impl std::error::Error for ThemeError {}

impl UiTheme {
    /// Reads the theme at `path`, through the asset funnel.
    pub fn load(path: &Path) -> Result<Self, ThemeError> {
        let text = voltra_assets::fs::read_to_string(path).map_err(ThemeError::Io)?;
        Self::parse(&text)
    }

    /// Reads a theme from RON `text`.
    pub fn parse(text: &str) -> Result<Self, ThemeError> {
        let theme: Self = ron::from_str(text).map_err(ThemeError::Parse)?;
        if theme.version > THEME_VERSION {
            return Err(ThemeError::Version(theme.version));
        }
        Ok(theme)
    }
}

#[cfg(test)]
mod tests;
