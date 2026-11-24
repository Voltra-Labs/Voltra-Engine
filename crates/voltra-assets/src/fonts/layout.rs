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

/// One character to place: its glyph, its advance, the kerning it takes
/// from the character before it, and whether a line may break at it.
struct Item {
    key: GlyphKey,
    advance: f32,
    kern: f32,
    space: bool,
}

/// Lays `content` out in `face` at `px` pixels per em, breaking at `\n`
/// and, with a width, between words.
///
/// Control characters other than `\n` are skipped rather than drawn as the
/// face's missing-glyph box: a `\r` from a Windows clipboard is not content.
///
/// Wrapping is greedy, as Godot's and Unity's are: as many words as fit, a
/// break at the last space before the one that does not, the spaces at the
/// break dropped. A word wider than the whole line is broken between
/// characters rather than left sticking out. A width that is not a positive
/// number wraps nothing.
pub fn layout(face: &fontdue::Font, content: &str, px: u32, wrap: Wrap) -> TextLayout {
    let size = px as f32;
    let (ascent, descent, line_height) = match face.horizontal_line_metrics(size) {
        Some(m) => (m.ascent, m.descent, m.new_line_size),
        // A face with no horizontal metrics still lays out: one em per line.
        None => (size, 0.0, size),
    };
    let spacing = if wrap.line_spacing.is_finite() && wrap.line_spacing > 0.0 {
        wrap.line_spacing
    } else {
        1.0
    };
    let max_width = wrap.max_width.filter(|w| w.is_finite() && *w > 0.0);

    let mut out = TextLayout {
        glyphs: Vec::new(),
        line_widths: Vec::new(),
        line_height: line_height * spacing,
        ascent,
        descent,
    };

    for paragraph in content.split('\n') {
        let mut items = Vec::new();
        let mut previous: Option<u16> = None;
        for character in paragraph.chars().filter(|c| !c.is_control()) {
            let glyph = face.lookup_glyph_index(character);
            let kern = previous
                .and_then(|left| face.horizontal_kern_indexed(left, glyph, size))
                .unwrap_or(0.0);
            items.push(Item {
                key: GlyphKey { glyph, px },
                advance: face.metrics_indexed(glyph, size).advance_width,
                kern,
                space: character.is_whitespace(),
            });
            previous = Some(glyph);
        }
        for range in breaks(&items, max_width) {
            let line = out.line_widths.len();
            let mut pen = 0.0;
            for (i, item) in items[range.clone()].iter().enumerate() {
                if i > 0 {
                    pen += item.kern;
                }
                out.glyphs.push(PlacedGlyph {
                    key: item.key,
                    pen,
                    line,
                });
                pen += item.advance;
            }
            out.line_widths.push(pen);
        }
    }
    out
}

/// The items of each line, greedily. Always at least one line, empty for
/// an empty paragraph.
fn breaks(items: &[Item], max_width: Option<f32>) -> Vec<std::ops::Range<usize>> {
    let Some(max) = max_width else {
        return std::iter::once(0..items.len()).collect();
    };
    let mut lines = Vec::new();
    let mut start = 0;
    loop {
        let mut width = 0.0;
        let mut last_space = None;
        let mut i = start;
        while i < items.len() {
            let item = &items[i];
            let step = item.advance + if i > start { item.kern } else { 0.0 };
            // Spaces never break the width: they are dropped at a break.
            if !item.space && i > start && width + step > max {
                break;
            }
            if item.space {
                last_space = Some(i);
            }
            width += step;
            i += 1;
        }
        if i == items.len() {
            lines.push(start..trim(items, start, i));
            return lines;
        }
        let end = match last_space {
            Some(space) if space > start => space,
            _ => i,
        };
        lines.push(start..trim(items, start, end));
        start = end;
        while start < items.len() && items[start].space {
            start += 1;
        }
        if start == items.len() {
            return lines;
        }
    }
}

/// `end` moved back over trailing spaces, but not past `start`.
fn trim(items: &[Item], start: usize, mut end: usize) -> usize {
    while end > start && items[end - 1].space {
        end -= 1;
    }
    end
}
