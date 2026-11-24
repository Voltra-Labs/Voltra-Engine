//! What a paragraph is laid out from: its text, cut into spans that each
//! say how their bytes are set.

use std::hash::{Hash, Hasher};
use std::ops::Range;

use super::font::Font;
use crate::handle::Handle;

/// How one span of a paragraph is set.
///
/// A plain text is one span; rich text's markup (`[b]`, `[size=24]`,
/// `[font=…]`, `[icon=…]`) is several. Colour is not here: it changes no
/// glyph, so the caller keeps it, by [`tag`](Self::tag).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RunStyle {
    /// The font tried first. The fallback chain
    /// ([`Fonts::set_fallbacks`](crate::Fonts::set_fallbacks), then the
    /// default font) supplies the glyphs it lacks.
    pub font: Handle<Font>,
    /// Raster size, in pixels per em.
    pub px: u32,
    /// Faux bold: thicken the glyphs, and widen their advance to match.
    pub embolden: bool,
    /// An inline box rather than glyphs — an icon. Its span's text is one
    /// character, conventionally U+FFFC, which bidi and line breaking treat
    /// as a neutral object.
    pub object: Option<InlineBox>,
    /// The caller's own index for the span, handed back on every glyph and
    /// box placed from it.
    pub tag: u32,
}

impl RunStyle {
    /// Glyphs of `font` at `px`, plain, tagged 0.
    pub fn plain(font: Handle<Font>, px: u32) -> Self {
        Self {
            font,
            px,
            embolden: false,
            object: None,
            tag: 0,
        }
    }
}

/// An inline box's size, in pixels: it sits on the baseline and advances
/// the pen by its width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InlineBox {
    /// Advance, in pixels.
    pub width: f32,
    /// Height above the baseline, in pixels.
    pub height: f32,
}

impl Eq for InlineBox {}

impl Hash for InlineBox {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.width.to_bits().hash(state);
        self.height.to_bits().hash(state);
    }
}

/// A run of a paragraph's bytes and how they are set.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Span {
    /// Byte range in the laid-out string. Spans are in order and do not
    /// overlap; bytes no span covers take the style of the span before
    /// them (or the first).
    pub range: Range<usize>,
    /// How they are set.
    pub style: RunStyle,
}

impl Span {
    /// The span covering all of `text`, in `style`.
    pub fn whole(text: &str, style: RunStyle) -> Self {
        Self {
            range: 0..text.len(),
            style,
        }
    }
}

/// The span index covering byte `at` of the text, as [`Span::range`] says
/// gaps are filled. `spans` is never empty here.
pub(crate) fn span_at(spans: &[Span], at: usize) -> usize {
    match spans.binary_search_by(|span| {
        if span.range.end <= at {
            std::cmp::Ordering::Less
        } else if span.range.start > at {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    }) {
        Ok(index) => index,
        Err(after) => after.saturating_sub(1).min(spans.len() - 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(range: Range<usize>, tag: u32) -> Span {
        Span {
            range,
            style: RunStyle {
                tag,
                ..RunStyle::plain(Handle::forge(0, 0), 16)
            },
        }
    }

    #[test]
    fn a_byte_finds_its_span_and_a_gap_takes_the_one_before() {
        let spans = [span(0..3, 0), span(5..8, 1)];
        assert_eq!(span_at(&spans, 0), 0);
        assert_eq!(span_at(&spans, 2), 0);
        assert_eq!(span_at(&spans, 3), 0, "a gap: the span before");
        assert_eq!(span_at(&spans, 6), 1);
        assert_eq!(span_at(&spans, 99), 1, "past the end: the last");
    }
}
