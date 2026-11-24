//! A [`Text`] as the layout takes it: the string drawn and the spans that
//! say how each part of it is set — one span for plain text, one per
//! styled run of rich text.

use std::borrow::Cow;

use voltra_assets::fonts::{InlineBox, RunStyle, Span};
use voltra_assets::{Font, Fonts, Handle};

use super::markup::{self, Style};
use super::Text;

/// A text ready to lay out.
#[derive(Debug, Clone, PartialEq)]
pub struct Paragraph<'a> {
    /// What is drawn: the content, its tags taken out.
    pub content: Cow<'a, str>,
    /// How each part of it is set; span `i` is tagged `i`.
    pub spans: Vec<Span>,
    /// Each span's rich-text style, by the same index: the colour and icon
    /// the draw needs and the layout does not.
    pub styles: Vec<Style>,
}

/// What `text` lays out as at `px` raster pixels per em, `font` being its
/// resolved face.
///
/// A `[font=…]` or a bold face not loaded yet — [`load_fonts`] runs in the
/// prepare, before any draw — sets its run in `font` meanwhile.
pub fn of<'a>(text: &'a Text, fonts: &Fonts, font: Handle<Font>, px: u32) -> Paragraph<'a> {
    if !text.rich || !text.content.contains('[') {
        let content = Cow::Borrowed(text.content.as_str());
        let spans = vec![Span::whole(&content, RunStyle::plain(font, px))];
        return Paragraph {
            content,
            spans,
            styles: vec![Style::default()],
        };
    }
    let parsed = markup::parse(&text.content);
    let mut icons = text.icons.iter();
    let spans = parsed
        .runs
        .iter()
        .enumerate()
        .map(|(i, run)| {
            let style = &run.style;
            let run_px = match style.size {
                Some(size) if text.size > 0.0 => {
                    let scaled = (px as f32 * size / text.size).round();
                    if scaled.is_finite() {
                        (scaled as u32).clamp(1, Text::MAX_PIXELS_PER_EM)
                    } else {
                        px
                    }
                }
                _ => px,
            };
            let named = style
                .font
                .as_ref()
                .and_then(|path| fonts.by_path_handle(path));
            let bold_face = (style.bold && style.font.is_none())
                .then_some(text.bold_font.as_ref())
                .flatten()
                .and_then(|path| fonts.by_path_handle(path));
            let run_font = bold_face.or(named).unwrap_or(font);
            let object = style.icon.as_ref().map(|reference| {
                let aspect = icons
                    .next()
                    .filter(|icon| icon.reference == *reference)
                    .map_or(1.0, |icon| icon.aspect);
                let height = fonts
                    .try_get(run_font)
                    .and_then(|f| f.line_metrics(run_px))
                    .map_or(run_px as f32, |m| m.ascent);
                InlineBox {
                    width: height * aspect,
                    height,
                }
            });
            Span {
                range: run.range.clone(),
                style: RunStyle {
                    font: run_font,
                    px: run_px,
                    embolden: style.bold && bold_face.is_none(),
                    object,
                    tag: i as u32,
                },
            }
        })
        .collect();
    Paragraph {
        styles: parsed.runs.into_iter().map(|run| run.style).collect(),
        content: Cow::Owned(parsed.text),
        spans,
    }
}

/// Loads the faces `text`'s rich text names — its bold face and every
/// `[font=…]` — so its runs can be set in them.
pub fn load_fonts(text: &Text, fonts: &mut Fonts) {
    if !text.rich || !text.content.contains('[') {
        return;
    }
    if let Some(bold) = &text.bold_font {
        fonts.load(bold);
    }
    for run in markup::parse(&text.content).runs {
        if let Some(path) = &run.style.font {
            fonts.load(path);
        }
    }
}

/// The colour a run of `text` styled `style` draws in: its own, or the
/// text's, faded by the text's alpha either way.
pub fn color(text: &Text, style: &Style) -> [f32; 4] {
    match style.color {
        Some([r, g, b, a]) => [r, g, b, a * text.color[3]],
        None => text.color,
    }
}
