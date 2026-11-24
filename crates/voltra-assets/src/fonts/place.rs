//! One line's glyphs put in visual order and placed, its caret stops and
//! its height measured.

use std::ops::Range;

use unicode_bidi::{Level, ParagraphBidiInfo};

use super::atlas::GlyphKey;
use super::breaks::Cluster;
use super::layout::{CaretStop, PlacedGlyph, PlacedObject, TextLayout};
use super::shape::Shaped;
use super::span::{span_at, Span};
use crate::fonts::Fonts;

/// What [`line`] needs of the paragraph a line is cut from.
pub(crate) struct Paragraph<'a> {
    /// The whole laid-out string.
    pub text: &'a str,
    /// Where the paragraph starts in it.
    pub base: usize,
    /// Its bidi analysis, over the paragraph's own bytes.
    pub bidi: &'a ParagraphBidiInfo<'a>,
    /// Its glyphs, in logical order.
    pub shaped: &'a [Shaped],
    /// Those glyphs' clusters.
    pub clusters: &'a [Cluster],
    /// The string's spans.
    pub spans: &'a [Span],
}

/// A placed line's measures, in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LineBox {
    /// Above the baseline: the tallest ascent of its faces and boxes.
    pub ascent: f32,
    /// Below it, negative: the lowest descent.
    pub descent: f32,
    /// The largest of its faces' own line heights.
    pub height: f32,
}

impl LineBox {
    /// The line gap its tallest face asks for on top of ascent and descent.
    pub fn gap(&self) -> f32 {
        (self.height - (self.ascent - self.descent)).max(0.0)
    }
}

/// Places the line made of `clusters` (a range of the paragraph's), whose
/// caret stops cover the bytes `bytes`, as line number
/// `out.line_widths.len()`; `last` when it ends its paragraph, which adds
/// the stop after the paragraph's last character.
pub(crate) fn line(
    fonts: &Fonts,
    para: &Paragraph<'_>,
    clusters: Range<usize>,
    bytes: Range<usize>,
    last: bool,
    out: &mut TextLayout,
) -> LineBox {
    let number = out.line_widths.len();
    let line_clusters = &para.clusters[clusters.clone()];
    let glyphs = match (line_clusters.first(), line_clusters.last()) {
        (Some(first), Some(end)) => first.glyphs.start..end.glyphs.end,
        _ => 0..0,
    };
    let rtl_paragraph = para.bidi.paragraph_level.is_rtl();

    // Visual order: L1 applied to this line's levels, then L2's reversals.
    let levels: Vec<Level> = if glyphs.is_empty() {
        Vec::new()
    } else {
        let from = line_clusters[0].bytes.start - para.base;
        let to = line_clusters[line_clusters.len() - 1].bytes.end - para.base;
        let line_levels = para.bidi.reordered_levels(from..to);
        para.shaped[glyphs.clone()]
            .iter()
            .map(|g| line_levels[g.cluster - para.base])
            .collect()
    };
    let order = ParagraphBidiInfo::reorder_visual(&levels);

    // Glyph `i` of the paragraph is in cluster `owner[i - glyphs.start]`
    // of the line.
    let mut owner = Vec::with_capacity(glyphs.len());
    for (c, cluster) in line_clusters.iter().enumerate() {
        owner.extend(std::iter::repeat_n(c, cluster.glyphs.len()));
    }
    let mut extents = vec![(f32::INFINITY, f32::NEG_INFINITY); line_clusters.len()];

    let mut pen = 0.0;
    let mut metrics = Metrics::default();
    for index in order {
        let g = &para.shaped[glyphs.start + index];
        let tag = para.spans[g.span].style.tag;
        let extent = &mut extents[owner[index]];
        extent.0 = extent.0.min(pen);
        extent.1 = extent.1.max(pen + g.advance);
        if g.object {
            let height = para.spans[g.span].style.object.map_or(0.0, |o| o.height);
            out.objects.push(PlacedObject {
                tag,
                pen,
                line: number,
                width: g.advance,
                height,
            });
            metrics.add_box(height);
        } else {
            out.glyphs.push(PlacedGlyph {
                key: GlyphKey {
                    font: g.font.index(),
                    glyph: g.glyph,
                    px: g.px,
                    embolden: g.embolden,
                },
                pen: pen + g.offset[0],
                rise: g.offset[1],
                line: number,
                tag,
                cluster: g.cluster,
            });
            metrics.add_face(fonts, g.font, g.px);
        }
        pen += g.advance;
    }
    out.line_widths.push(pen);

    if metrics.empty {
        // An empty line is as tall as the font its first byte is set in.
        let style = para.spans[span_at(para.spans, bytes.start)].style;
        if let Some(font) = fonts.first_loaded(style.font) {
            metrics.add_face(fonts, font, style.px);
        }
    }

    carets(
        para,
        line_clusters,
        &extents,
        bytes,
        last,
        rtl_paragraph,
        pen,
        number,
        out,
    );
    LineBox {
        ascent: metrics.ascent,
        descent: metrics.descent,
        height: metrics.height,
    }
}

/// Every caret stop of a line: the leading edge of each character —
/// left in left-to-right text, right in right-to-left — a cluster's width
/// shared among its characters (an `ffi` ligature has three stops inside
/// one glyph, as browsers give it).
#[allow(clippy::too_many_arguments)]
fn carets(
    para: &Paragraph<'_>,
    clusters: &[Cluster],
    extents: &[(f32, f32)],
    bytes: Range<usize>,
    last: bool,
    rtl_paragraph: bool,
    width: f32,
    line: usize,
    out: &mut TextLayout,
) {
    // A character in no drawn cluster — white space hung at a break, a
    // control character — stands at the line's far end.
    let far = if rtl_paragraph { 0.0 } else { width };
    let rtl_at = |c: &Cluster| {
        para.shaped
            .get(c.glyphs.start)
            .is_some_and(|g| g.level.is_rtl())
    };
    let edge = |byte: usize, trailing: bool| -> f32 {
        let Ok(c) = clusters.binary_search_by(|cluster| {
            if cluster.bytes.end <= byte {
                std::cmp::Ordering::Less
            } else if cluster.bytes.start > byte {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        }) else {
            return far;
        };
        let cluster = &clusters[c];
        let (left, right) = extents[c];
        if !left.is_finite() {
            return far;
        }
        let chars = para.text[cluster.bytes.clone()].char_indices();
        let count = chars.clone().count().max(1) as f32;
        let k = chars
            .take_while(|(i, _)| cluster.bytes.start + i < byte)
            .count() as f32;
        let share = if trailing {
            (k + 1.0) / count
        } else {
            k / count
        };
        if rtl_at(cluster) {
            right - (right - left) * share
        } else {
            left + (right - left) * share
        }
    };
    let text = &para.text[bytes.clone()];
    for (offset, _) in text.char_indices() {
        let byte = bytes.start + offset;
        out.carets.push(CaretStop {
            byte,
            x: edge(byte, false),
            line,
        });
    }
    if last {
        let x = match text.char_indices().next_back() {
            Some((offset, _)) => edge(bytes.start + offset, true),
            None => far,
        };
        out.carets.push(CaretStop {
            byte: bytes.end,
            x,
            line,
        });
    }
}

/// A line's measures as its glyphs are added.
struct Metrics {
    ascent: f32,
    descent: f32,
    height: f32,
    empty: bool,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            ascent: 0.0,
            descent: 0.0,
            height: 0.0,
            empty: true,
        }
    }
}

impl Metrics {
    fn add_face(&mut self, fonts: &Fonts, font: crate::Handle<crate::Font>, px: u32) {
        let Some(m) = fonts.try_get(font).and_then(|f| f.line_metrics(px)) else {
            return;
        };
        self.merge(m.ascent, m.descent, m.line_height);
    }

    fn add_box(&mut self, height: f32) {
        self.merge(height, 0.0, height);
    }

    fn merge(&mut self, ascent: f32, descent: f32, height: f32) {
        if self.empty {
            *self = Self {
                ascent,
                descent,
                height,
                empty: false,
            };
            return;
        }
        self.ascent = self.ascent.max(ascent);
        self.descent = self.descent.min(descent);
        self.height = self.height.max(height);
    }
}
