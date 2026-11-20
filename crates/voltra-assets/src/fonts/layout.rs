//! Placing a string's glyphs along baselines, in raster pixels.
//!
//! Ours rather than `fontdue::layout::Layout`: that one applies no kerning,
//! aligns only inside a `max_width` box, and works in a y-down/y-up mode
//! switch. What a world-space label needs is smaller — pens along lines split
//! at `\n`, kerned, with each line's width so the caller can align about its
//! own origin.

use super::atlas::GlyphKey;

/// One glyph placed on a line: the pen position it starts at, in pixels from
/// the line's left end along its baseline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedGlyph {
    pub key: GlyphKey,
    pub pen: f32,
    pub line: usize,
}

/// A laid-out string. Lines run downward from the first baseline, one
/// [`line_height`](Self::line_height) apart.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextLayout {
    pub glyphs: Vec<PlacedGlyph>,
    /// Each line's advance width, in pixels. One entry per line, even an
    /// empty one, so a trailing `\n` still counts as a line.
    pub line_widths: Vec<f32>,
    /// Baseline to baseline, in pixels.
    pub line_height: f32,
    /// Baseline to the top of the tallest glyph the face declares, in pixels.
    pub ascent: f32,
    /// Baseline to the bottom of the lowest glyph the face declares, in
    /// pixels. Negative: below the baseline is down.
    pub descent: f32,
}

impl TextLayout {
    /// The widest line, in pixels.
    pub fn width(&self) -> f32 {
        self.line_widths.iter().copied().fold(0.0, f32::max)
    }
}

/// Lays `content` out in `face` at `px` pixels per em.
///
/// Control characters other than `\n` are skipped rather than drawn as the
/// face's missing-glyph box: a `\r` from a Windows clipboard is not content.
pub fn layout(face: &fontdue::Font, content: &str, px: u32) -> TextLayout {
    let size = px as f32;
    let (ascent, descent, line_height) = match face.horizontal_line_metrics(size) {
        Some(m) => (m.ascent, m.descent, m.new_line_size),
        // A face with no horizontal metrics still lays out: one em per line.
        None => (size, 0.0, size),
    };

    let mut out = TextLayout {
        glyphs: Vec::new(),
        line_widths: Vec::new(),
        line_height,
        ascent,
        descent,
    };

    for (line, text) in content.split('\n').enumerate() {
        let mut pen = 0.0;
        let mut previous: Option<u16> = None;
        for character in text.chars().filter(|c| !c.is_control()) {
            let glyph = face.lookup_glyph_index(character);
            if let Some(left) = previous {
                pen += face
                    .horizontal_kern_indexed(left, glyph, size)
                    .unwrap_or(0.0);
            }
            out.glyphs.push(PlacedGlyph {
                key: GlyphKey { glyph, px },
                pen,
                line,
            });
            pen += face.metrics_indexed(glyph, size).advance_width;
            previous = Some(glyph);
        }
        out.line_widths.push(pen);
    }
    out
}
