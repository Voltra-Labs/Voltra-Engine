//! A string's glyphs placed along baselines, in raster pixels.
//!
//! Ours rather than `fontdue::layout::Layout`, which applies no shaping,
//! aligns only inside a `max_width` box and knows one direction. Each
//! paragraph (split at `\n`) goes through the steps every text engine has —
//! HarfBuzz's own users (Chrome, Godot's `TextServerAdvanced`, cosmic-text)
//! in the same order:
//!
//! 1. bidi levels (UAX #9, `unicode-bidi`);
//! 2. runs of one span, one level, one script ([`itemize`](super::itemize));
//! 3. each run shaped, the fallback chain filling missing glyphs
//!    ([`shape`](super::shape));
//! 4. lines cut at UAX #14 opportunities ([`breaks`](super::breaks));
//! 5. each line's glyphs reordered visually by level, and placed
//!    ([`place`](super::place)).

use std::hash::{Hash, Hasher};

use unicode_bidi::ParagraphBidiInfo;

use super::atlas::GlyphKey;
use super::breaks;
use super::itemize;
use super::place::{self, LineBox, Paragraph};
use super::shape::{shape_run, Faces};
use super::span::Span;
use crate::fonts::Fonts;

/// One glyph placed on a line, in visual order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedGlyph {
    /// Which glyph, of which font, at what size.
    pub key: GlyphKey,
    /// Where its origin is along the line's baseline, in pixels from the
    /// line's left end: the pen, plus the shaper's offset (a mark sits over
    /// its base this way).
    pub pen: f32,
    /// Its origin's height above the baseline, in pixels: the shaper's
    /// vertical offset, zero for most glyphs.
    pub rise: f32,
    /// Zero-based index of the line it is on.
    pub line: usize,
    /// The [`tag`](super::RunStyle::tag) of the span it was set in.
    pub tag: u32,
    /// The first byte of the characters it was shaped from.
    pub cluster: usize,
}

/// An inline box placed on a line — an icon in rich text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedObject {
    /// The [`tag`](super::RunStyle::tag) of its span.
    pub tag: u32,
    /// Its left edge along the baseline, in pixels from the line's left end.
    pub pen: f32,
    /// Zero-based index of the line it is on.
    pub line: usize,
    /// Its size, in pixels; it stands on the baseline.
    pub width: f32,
    /// Height above the baseline, in pixels.
    pub height: f32,
}

/// Where a caret stands before one character, or after a paragraph's last.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaretStop {
    /// The byte it stands before, in the laid-out string.
    pub byte: usize,
    /// Its x, in pixels from the line's left end.
    pub x: f32,
    /// The line it is on.
    pub line: usize,
}

/// A laid-out string. Lines run downward from the first baseline.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextLayout {
    /// The glyphs, each placed on its line, left to right.
    pub glyphs: Vec<PlacedGlyph>,
    /// The inline boxes, each placed on its line.
    pub objects: Vec<PlacedObject>,
    /// Each line's advance width, in pixels. One entry per line, even an
    /// empty one, so a trailing `\n` still counts as a line.
    pub line_widths: Vec<f32>,
    /// Each line's baseline, in pixels below the first one: `0.0` first.
    /// Lines of one face and size are [`line_height`](Self::line_height)
    /// apart; a line with a taller font or a larger `[size]` is further.
    pub baselines: Vec<f32>,
    /// The first line's baseline-to-baseline height, in pixels.
    pub line_height: f32,
    /// The first line's baseline to the top of its tallest face's glyphs,
    /// in pixels.
    pub ascent: f32,
    /// The last line's baseline to the bottom of its lowest face's glyphs,
    /// in pixels. Negative: below the baseline is down.
    pub descent: f32,
    /// Where a caret may stand: before every character, in the string's
    /// order, and after each paragraph's last. Right to left text puts the
    /// stop of a character on its right.
    pub carets: Vec<CaretStop>,
}

impl TextLayout {
    /// The widest line, in pixels.
    pub fn width(&self) -> f32 {
        self.line_widths.iter().copied().fold(0.0, f32::max)
    }

    /// The last line's baseline, in pixels below the first.
    pub fn last_baseline(&self) -> f32 {
        self.baselines.last().copied().unwrap_or(0.0)
    }
}

/// How lines break and how far apart they are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wrap {
    /// The widest a line may be, in pixels: words that would pass it start
    /// a new line. `None` breaks only at `\n`.
    pub max_width: Option<f32>,
    /// Baseline to baseline, as a share of the face's own line height.
    pub line_spacing: f32,
}

impl Default for Wrap {
    fn default() -> Self {
        Self {
            max_width: None,
            line_spacing: 1.0,
        }
    }
}

impl Eq for Wrap {}

impl Hash for Wrap {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.max_width.map(f32::to_bits).hash(state);
        self.line_spacing.to_bits().hash(state);
    }
}

/// Lays out `text`, set as `spans` say, broken as `wrap` says. `None` when
/// no font of the first span's chain has a face.
///
/// A width that is not a positive number wraps nothing; a spacing that is
/// not one is 1.
pub(crate) fn layout(fonts: &Fonts, text: &str, spans: &[Span], wrap: Wrap) -> Option<TextLayout> {
    let first = spans.first()?;
    fonts.first_loaded(first.style.font)?;
    let spacing = if wrap.line_spacing.is_finite() && wrap.line_spacing > 0.0 {
        wrap.line_spacing
    } else {
        1.0
    };
    let max_width = wrap.max_width.filter(|w| w.is_finite() && *w > 0.0);

    let lookup = |handle| fonts.try_get(handle);
    let mut faces = Faces::new(&lookup);
    let mut out = TextLayout::default();
    let mut boxes: Vec<LineBox> = Vec::new();

    let mut base = 0;
    for paragraph in text.split('\n') {
        let bidi = ParagraphBidiInfo::new(paragraph, None);
        let runs = itemize::runs(paragraph, base, spans, &bidi.levels);
        let shaped: Vec<_> = runs
            .iter()
            .flat_map(|run| {
                let chain = fonts.chain(spans[run.span].style.font);
                shape_run(&mut faces, text, run, spans, &chain)
            })
            .collect();
        let end = base + paragraph.len();
        let clusters = breaks::clusters(text, &shaped, end);
        let lines = breaks::lines(paragraph, base, &clusters, max_width);
        let para = Paragraph {
            text,
            base,
            bidi: &bidi,
            shaped: &shaped,
            clusters: &clusters,
            spans,
        };
        for (k, line) in lines.iter().enumerate() {
            let start = if k == 0 {
                base
            } else {
                clusters.get(line.start).map_or(end, |c| c.bytes.start)
            };
            let stop = lines
                .get(k + 1)
                .and_then(|next| clusters.get(next.start))
                .map_or(end, |c| c.bytes.start);
            let last = k + 1 == lines.len();
            boxes.push(place::line(
                fonts,
                &para,
                line.clone(),
                start..stop,
                last,
                &mut out,
            ));
        }
        base = end + 1;
    }

    // Baselines: the gap below one line's lowest descent to the next's
    // highest ascent, plus the larger line gap of the two — a face's own
    // line height when every line is one face and size.
    let mut baseline = 0.0;
    for (i, line) in boxes.iter().enumerate() {
        if i > 0 {
            let above = &boxes[i - 1];
            let gap = above.gap().max(line.gap());
            baseline += (-above.descent + line.ascent + gap) * spacing;
        }
        out.baselines.push(baseline);
    }
    if let (Some(first), Some(last)) = (boxes.first(), boxes.last()) {
        out.line_height = first.height * spacing;
        out.ascent = first.ascent;
        out.descent = last.descent;
    }
    Some(out)
}
