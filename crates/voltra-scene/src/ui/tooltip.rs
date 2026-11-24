//! A tooltip: a line of help shown by an element after a hover, or while it
//! has the focus.

use voltra_assets::{AssetPath, Fonts};

use crate::Text;

/// Help text an element shows in a box beside it once the pointer has
/// rested on it — or on anything under it — for [`delay`](Self::delay)
/// seconds, and as long as it holds the keyboard or pad focus, after the
/// same delay: Godot's `tooltip_text`, which a pad player also gets.
///
/// Drawn by the UI pass over every element, in a box under the element
/// (over it when there is no room below), kept on the screen. The text is
/// rich text, wrapped at [`max_width`](Self::max_width).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UiTooltip {
    /// What it says.
    #[serde(default)]
    pub text: String,
    /// A string-table key the text comes from, if it is translated, as
    /// [`Text::key`](crate::Text::key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Seconds of hover, or of focus, before it shows.
    #[serde(default = "half")]
    pub delay: f32,
    /// The widest its text runs before wrapping, in points.
    #[serde(default = "wide")]
    pub max_width: f32,
    /// Its text's face, by asset path; `None` — the default, and not
    /// written — is the default font.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<AssetPath>,
    /// The em height of its text, in points.
    #[serde(default = "small")]
    pub size: f32,
    /// Behind the text.
    #[serde(default = "background")]
    pub background: [f32; 4],
    /// The text's colour.
    #[serde(default = "white")]
    pub color: [f32; 4],
    /// Whether it shows now, as the last update left it.
    #[serde(skip)]
    pub shown: bool,
}

fn half() -> f32 {
    0.5
}

fn wide() -> f32 {
    260.0
}

fn small() -> f32 {
    14.0
}

fn background() -> [f32; 4] {
    [0.02, 0.02, 0.04, 0.94]
}

fn white() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

impl Default for UiTooltip {
    fn default() -> Self {
        Self {
            text: "Help".to_owned(),
            key: None,
            delay: half(),
            max_width: wide(),
            font: None,
            size: small(),
            background: background(),
            color: white(),
            shown: false,
        }
    }
}

impl UiTooltip {
    /// A tooltip saying `text`, every other field at its default.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    /// The text it is drawn with: its words in its face, size and colour,
    /// the face resolved against `fonts` if it is loaded.
    pub fn as_text(&self, fonts: &Fonts) -> Text {
        let font_handle = Some(
            self.font
                .as_ref()
                .and_then(|path| fonts.by_path_handle(path))
                .unwrap_or_else(|| fonts.default_font()),
        );
        Text {
            font: self.font.clone(),
            font_handle,
            size: self.size,
            color: self.color,
            max_width: Some(self.max_width.max(1.0)),
            ..Text::new(self.text.clone())
        }
    }
}
