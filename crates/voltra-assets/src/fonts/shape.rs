//! One run shaped into glyphs, the fallback chain filling what its first
//! font lacks.

use std::ops::Range;

use unicode_bidi::Level;

use super::embolden;
use super::font::Font;
use super::itemize::Run;
use super::span::Span;
use crate::handle::Handle;

/// A glyph the shaper chose, in logical order, before lines are cut.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Shaped {
    /// The font it is drawn from: the run's own, or a fallback.
    pub font: Handle<Font>,
    /// Its id in that font.
    pub glyph: u16,
    /// The first byte of the cluster it belongs to, in the whole string.
    pub cluster: usize,
    /// How far it moves the pen, in pixels.
    pub advance: f32,
    /// Where it is drawn relative to the pen, in pixels, y up.
    pub offset: [f32; 2],
    /// Index of the span it was set in.
    pub span: usize,
    /// Its run's bidi level.
    pub level: Level,
    /// Raster size.
    pub px: u32,
    /// Faux bold.
    pub embolden: bool,
    /// An inline box, not a glyph: `glyph` means nothing.
    pub object: bool,
}

/// The shaping faces of a layout's fonts, made once per layout: a
/// `rustybuzz::Face` borrows its font's bytes, so it cannot be kept in the
/// font itself.
pub(crate) struct Faces<'a> {
    faces: Vec<(Handle<Font>, Option<rustybuzz::Face<'a>>)>,
    lookup: &'a dyn Fn(Handle<Font>) -> Option<&'a Font>,
}

impl<'a> Faces<'a> {
    pub(crate) fn new(lookup: &'a dyn Fn(Handle<Font>) -> Option<&'a Font>) -> Self {
        Self {
            faces: Vec::new(),
            lookup,
        }
    }

    fn get(&mut self, font: Handle<Font>) -> Option<&rustybuzz::Face<'a>> {
        let index = match self.faces.iter().position(|(h, _)| *h == font) {
            Some(index) => index,
            None => {
                let face = (self.lookup)(font).and_then(Font::shaper);
                self.faces.push((font, face));
                self.faces.len() - 1
            }
        };
        self.faces[index].1.as_ref()
    }
}

/// Shapes `run` of `text` with the first font of `chain` that has a face,
/// and each stretch of clusters it has no glyph for with the next — the
/// browsers' and Godot's per-cluster fallback. What no font has is drawn as
/// the last font's missing-glyph box, so a gap shows where it is missing.
///
/// Control characters are left out — a `\r` from a Windows clipboard is not
/// content — but keep their bytes, so clusters still index `text`.
pub(crate) fn shape_run(
    faces: &mut Faces<'_>,
    text: &str,
    run: &Run,
    spans: &[Span],
    chain: &[Handle<Font>],
) -> Vec<Shaped> {
    let style = spans[run.span].style;
    if let Some(object) = style.object {
        return vec![Shaped {
            font: style.font,
            glyph: 0,
            cluster: run.range.start,
            advance: object.width,
            offset: [0.0, 0.0],
            span: run.span,
            level: run.level,
            px: style.px,
            embolden: false,
            object: true,
        }];
    }
    let fonts: Vec<Handle<Font>> = chain
        .iter()
        .copied()
        .filter(|font| faces.get(*font).is_some())
        .collect();
    let mut out = Vec::new();
    shape_with(faces, text, run, spans, &fonts, run.range.clone(), &mut out);
    out
}

fn shape_with(
    faces: &mut Faces<'_>,
    text: &str,
    run: &Run,
    spans: &[Span],
    chain: &[Handle<Font>],
    range: Range<usize>,
    out: &mut Vec<Shaped>,
) {
    let Some((&font, rest)) = chain.split_first() else {
        return;
    };
    let Some(face) = faces.get(font) else {
        return;
    };
    let glyphs = shape_once(face, font, text, run, spans, range.clone());
    if rest.is_empty() {
        out.extend(glyphs);
        return;
    }

    // Clusters in logical order; a cluster is missing when any of its
    // glyphs is the missing glyph.
    let mut i = 0;
    while i < glyphs.len() {
        let cluster = glyphs[i].cluster;
        let end = glyphs[i..]
            .iter()
            .position(|g| g.cluster != cluster)
            .map_or(glyphs.len(), |n| i + n);
        if !glyphs[i..end].iter().any(|g| g.glyph == 0) {
            out.extend_from_slice(&glyphs[i..end]);
            i = end;
            continue;
        }
        // A stretch of missing clusters, reshaped together so the next
        // font sees whole words to join and reorder.
        let mut stop = end;
        while stop < glyphs.len() {
            let next = glyphs[stop].cluster;
            let next_end = glyphs[stop..]
                .iter()
                .position(|g| g.cluster != next)
                .map_or(glyphs.len(), |n| stop + n);
            if !glyphs[stop..next_end].iter().any(|g| g.glyph == 0) {
                break;
            }
            stop = next_end;
        }
        let bytes = cluster..glyphs.get(stop).map_or(range.end, |g| g.cluster);
        if covered(faces, rest, &text[bytes.clone()]) {
            shape_with(faces, text, run, spans, rest, bytes, out);
        } else {
            // No later font has it either: this font's own missing-glyph
            // box, the run's font's look, rather than the last fallback's.
            out.extend_from_slice(&glyphs[i..stop]);
        }
        i = stop;
    }
}

/// Whether any font of `chain` maps a character of `stretch` that is not
/// white space or a mark — whether falling back is worth a reshape.
fn covered(faces: &mut Faces<'_>, chain: &[Handle<Font>], stretch: &str) -> bool {
    let wanted: Vec<char> = stretch
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect();
    chain.iter().any(|font| {
        faces
            .get(*font)
            .is_some_and(|face| wanted.iter().any(|c| face.glyph_index(*c).is_some()))
    })
}

/// `range` of `text` shaped by one face, in logical order.
fn shape_once(
    face: &rustybuzz::Face<'_>,
    font: Handle<Font>,
    text: &str,
    run: &Run,
    spans: &[Span],
    range: Range<usize>,
) -> Vec<Shaped> {
    let style = spans[run.span].style;
    let rtl = run.level.is_rtl();
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.set_pre_context(&text[..range.start]);
    for (offset, character) in text[range.clone()].char_indices() {
        if !character.is_control() {
            buffer.add(character, (range.start + offset) as u32);
        }
    }
    buffer.set_post_context(&text[range.end..]);
    if let Some(script) = run.script {
        buffer.set_script(script);
    }
    buffer.set_direction(if rtl {
        rustybuzz::Direction::RightToLeft
    } else {
        rustybuzz::Direction::LeftToRight
    });
    buffer.guess_segment_properties();

    let shaped = rustybuzz::shape(face, &[], buffer);
    let scale = style.px as f32 / face.units_per_em().max(1) as f32;
    let bold = if style.embolden {
        embolden::strength(style.px) as f32
    } else {
        0.0
    };
    let mut glyphs: Vec<Shaped> = shaped
        .glyph_infos()
        .iter()
        .zip(shaped.glyph_positions())
        .map(|(info, position)| Shaped {
            font,
            glyph: info.glyph_id as u16,
            cluster: info.cluster as usize,
            advance: position.x_advance as f32 * scale
                + if position.x_advance != 0 { bold } else { 0.0 },
            offset: [
                position.x_offset as f32 * scale,
                position.y_offset as f32 * scale,
            ],
            span: run.span,
            level: run.level,
            px: style.px,
            embolden: style.embolden,
            object: false,
        })
        .collect();
    // HarfBuzz hands a right-to-left run back in visual order; the line
    // breaker wants logical, and the bidi reorder turns it back.
    if rtl {
        glyphs.reverse();
    }
    glyphs
}
