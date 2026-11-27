//! Rich text: the inline tags a [`Text`](super::Text)'s content may carry,
//! parsed into the string drawn and the styled runs over it.
//!
//! BBCode-like, as Godot's `RichTextLabel` and Unity's TextMeshPro both
//! are: `[b]bold[/b]`, `[color=#f80]`, `[size=24]`, `[font=fonts/x.ttf]`,
//! and `[icon=…]`, which stands for an inline image and closes nothing.
//! `[[` is a literal `[`. A tag this parser does not know, a value it cannot
//! read, or a close with nothing open is shown as the text it is — a typo
//! is seen where it is — and logged once per spelling.

use std::collections::HashSet;
use std::ops::Range;
use std::sync::{Mutex, OnceLock};

use voltra_assets::AssetPath;

/// The largest `[size=…]`, in the text's own units: a tag asking for more
/// — `[size=1e30]` in a name a peer typed — is shown as written, not laid
/// out past every float a renderer can place.
pub const MAX_SIZE: f32 = 4096.0;

/// The character an inline icon stands on in the parsed string: the object
/// replacement character, which bidi and line breaking treat as one
/// neutral object.
pub const ICON_CHAR: char = '\u{FFFC}';

/// How a run of rich text differs from its text's own style.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Style {
    /// `[color=…]`: replaces the text's colour; its alpha is multiplied
    /// with the text's, so fading the text fades every colour in it.
    pub color: Option<[f32; 4]>,
    /// `[b]`: the text's bold face, or faux bold.
    pub bold: bool,
    /// `[size=…]`: the em height, in the text's own units; at most
    /// [`MAX_SIZE`].
    pub size: Option<f32>,
    /// `[font=…]`: another face, by asset path.
    pub font: Option<AssetPath>,
    /// `[icon=…]`: an inline image, by reference — `atlas.atlas.ron/3` for
    /// frame 3 of an atlas (drawn from the texture the atlas names), or an
    /// image file for all of it.
    pub icon: Option<String>,
}

/// A run of the parsed string and its style.
#[derive(Debug, Clone, PartialEq)]
pub struct Styled {
    /// Bytes of [`Markup::text`].
    pub range: Range<usize>,
    /// How they are set.
    pub style: Style,
}

/// A content parsed: what is drawn, and how each part of it is styled.
#[derive(Debug, Clone, PartialEq)]
pub struct Markup {
    /// The content with its tags taken out, an [`ICON_CHAR`] per icon.
    pub text: String,
    /// Runs covering all of `text`, in order; one, unstyled, for plain
    /// text.
    pub runs: Vec<Styled>,
}

/// One tag still open.
#[derive(Debug, Clone, PartialEq)]
enum Open {
    Bold,
    Color([f32; 4]),
    Size(f32),
    Font(AssetPath),
}

impl Open {
    fn name(&self) -> &'static str {
        match self {
            Self::Bold => "b",
            Self::Color(_) => "color",
            Self::Size(_) => "size",
            Self::Font(_) => "font",
        }
    }
}

/// What one `[…]` turned out to be.
enum Tag {
    Open(Open),
    Close(&'static str),
    Icon(String),
}

/// `text` to show as written in a rich text: each `[` doubled, so a name
/// or a message someone else typed is never read as tags (a colour, a
/// size, a font or an icon loaded from a path of their choosing). Plain
/// text — no `[` — is borrowed as it is.
pub fn escape(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains('[') {
        std::borrow::Cow::Owned(text.replace('[', "[["))
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}

/// Parses `source`'s tags. Plain text — no `[` — costs one scan.
pub fn parse(source: &str) -> Markup {
    if !source.contains('[') {
        return Markup {
            text: source.to_owned(),
            runs: vec![Styled {
                range: 0..source.len(),
                style: Style::default(),
            }],
        };
    }
    let mut text = String::with_capacity(source.len());
    let mut runs: Vec<Styled> = Vec::new();
    let mut open: Vec<Open> = Vec::new();
    let mut style = Style::default();
    let mut rest = source;

    let push = |text: &mut String, runs: &mut Vec<Styled>, piece: &str, style: &Style| {
        if piece.is_empty() {
            return;
        }
        let start = text.len();
        text.push_str(piece);
        match runs.last_mut() {
            Some(last) if last.style == *style && last.range.end == start => {
                last.range.end = text.len();
            }
            _ => runs.push(Styled {
                range: start..text.len(),
                style: style.clone(),
            }),
        }
    };

    while let Some(at) = rest.find('[') {
        push(&mut text, &mut runs, &rest[..at], &style);
        let after = &rest[at + 1..];
        if let Some(escaped) = after.strip_prefix('[') {
            push(&mut text, &mut runs, "[", &style);
            rest = escaped;
            continue;
        }
        let Some(end) = after.find(']') else {
            push(&mut text, &mut runs, &rest[at..], &style);
            rest = "";
            break;
        };
        let body = &after[..end];
        let whole = &rest[at..at + end + 2];
        match tag(body, &open) {
            Some(Tag::Open(tag)) => open.push(tag),
            Some(Tag::Close(name)) => {
                if let Some(i) = open.iter().rposition(|t| t.name() == name) {
                    open.remove(i);
                }
            }
            Some(Tag::Icon(reference)) => {
                let icon = Style {
                    icon: Some(reference),
                    ..style.clone()
                };
                let mut buffer = [0; 4];
                push(
                    &mut text,
                    &mut runs,
                    ICON_CHAR.encode_utf8(&mut buffer),
                    &icon,
                );
            }
            None => {
                warn_once(whole);
                push(&mut text, &mut runs, whole, &style);
            }
        }
        style = styled(&open);
        rest = &after[end + 1..];
    }
    push(&mut text, &mut runs, rest, &style);
    if runs.is_empty() {
        runs.push(Styled {
            range: 0..0,
            style: Style::default(),
        });
    }
    Markup { text, runs }
}

/// The style every tag still open makes, applied in the order they opened.
fn styled(open: &[Open]) -> Style {
    let mut style = Style::default();
    for tag in open {
        match tag {
            Open::Bold => style.bold = true,
            Open::Color(color) => style.color = Some(*color),
            Open::Size(size) => style.size = Some(*size),
            Open::Font(font) => style.font = Some(font.clone()),
        }
    }
    style
}

/// What `body` (between the brackets) is, or `None` to show it as text.
fn tag(body: &str, open: &[Open]) -> Option<Tag> {
    if let Some(name) = body.strip_prefix('/') {
        let name = match name.trim() {
            "b" => "b",
            "color" => "color",
            "size" => "size",
            "font" => "font",
            _ => return None,
        };
        // A close with nothing of its kind open is a typo, shown.
        return open
            .iter()
            .any(|t| t.name() == name)
            .then_some(Tag::Close(name));
    }
    let (name, value) = match body.split_once('=') {
        Some((name, value)) => (name.trim(), Some(value.trim())),
        None => (body.trim(), None),
    };
    Some(match (name, value) {
        ("b", None) => Tag::Open(Open::Bold),
        ("color", Some(value)) => Tag::Open(Open::Color(color(value)?)),
        ("size", Some(value)) => {
            let size: f32 = value.parse().ok()?;
            if !(size.is_finite() && size > 0.0 && size <= MAX_SIZE) {
                return None;
            }
            Tag::Open(Open::Size(size))
        }
        ("font", Some(value)) => Tag::Open(Open::Font(AssetPath::new(value).ok()?)),
        ("icon", Some(value)) if !value.is_empty() => Tag::Icon(value.to_owned()),
        _ => return None,
    })
}

/// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`, or one of a few names.
fn color(value: &str) -> Option<[f32; 4]> {
    let named = match value.to_ascii_lowercase().as_str() {
        "white" => Some([1.0, 1.0, 1.0, 1.0]),
        "black" => Some([0.0, 0.0, 0.0, 1.0]),
        "red" => Some([1.0, 0.0, 0.0, 1.0]),
        "green" => Some([0.0, 1.0, 0.0, 1.0]),
        "blue" => Some([0.0, 0.0, 1.0, 1.0]),
        "yellow" => Some([1.0, 1.0, 0.0, 1.0]),
        "gray" | "grey" => Some([0.5, 0.5, 0.5, 1.0]),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    let hex = value.strip_prefix('#')?;
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let digits: Vec<u8> = hex
        .chars()
        .map(|c| c.to_digit(16).map_or(0, |d| d as u8))
        .collect();
    let channels: Vec<f32> = match digits.len() {
        3 | 4 => digits.iter().map(|d| f32::from(d * 17) / 255.0).collect(),
        6 | 8 => digits
            .chunks(2)
            .map(|pair| f32::from(pair[0] * 16 + pair[1]) / 255.0)
            .collect(),
        _ => return None,
    };
    Some([
        channels[0],
        channels[1],
        channels[2],
        channels.get(3).copied().unwrap_or(1.0),
    ])
}

/// Logs a tag shown as text, once per spelling for the session.
fn warn_once(tag: &str) {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    let first = seen
        .lock()
        .map_or(true, |mut seen| seen.insert(tag.to_owned()));
    if first {
        log::warn!("{tag} is not a tag rich text knows; shown as written");
    }
}

#[cfg(test)]
mod tests;
