//! Where a [`Text`]'s glyphs land in its entity's space, and what they
//! sample.
//!
//! One function for the draw and the pick, for the reason
//! [`quad`](crate::sprite::quad::quad) is one: a click must land on the
//! pixels it appears to.

use std::sync::Arc;

use voltra_assets::fonts::TextLayout;
use voltra_assets::{Fonts, Handle};
use voltra_render::glam::Vec2;
use voltra_render::Texture;

use super::paragraph;
use super::Text;

/// One glyph's quad, in the entity's space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphQuad {
    /// Top-left, bottom-left, bottom-right, top-right, as the batch takes
    /// them.
    pub corners: [Vec2; 4],
    /// Texture coordinates for each corner, in the same order.
    pub uvs: [[f32; 2]; 4],
    /// Its colour: the text's, or its span's.
    pub color: [f32; 4],
}

/// A text's laid-out box, in the entity's space: from the leftmost line's
/// start to the rightmost line's end, and from the first line's ascent to
/// the last line's descent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    /// The corner with the smallest x and y.
    pub min: Vec2,
    /// The corner with the largest x and y.
    pub max: Vec2,
}

impl Bounds {
    /// Whether `local` is inside, edges included. A box with no width — no
    /// content — contains nothing.
    pub fn contains(&self, local: Vec2) -> bool {
        self.max.x > self.min.x
            && local.x >= self.min.x
            && local.x <= self.max.x
            && local.y >= self.min.y
            && local.y <= self.max.y
    }
}

/// Entity units per raster pixel, rasterized at `px`.
fn scale(text: &Text, px: u32) -> f32 {
    text.size / px.max(1) as f32
}

/// How far a line of `width` pixels is shifted to put its aligned end on
/// the origin.
fn offset(text: &Text, width: f32) -> f32 {
    -width * text.align.share()
}

/// `text` laid out at `px` raster pixels per em, wrapped at `width` entity
/// units, or `None` with no usable font. Shaped, bidi-ordered and broken by
/// [`Fonts::layout`], which caches it: the prepare, the draw and the pick
/// of one frame share one layout.
pub fn layout_at(
    text: &Text,
    fonts: &Fonts,
    px: u32,
    width: Option<f32>,
) -> Option<Arc<TextLayout>> {
    let font = text.font_handle?;
    let (content, spans) = paragraph::spans(text, fonts, font, px);
    fonts.layout(&content, &spans, text.wrap_at(px, width))
}

/// The laid-out box, or `None` with no usable font.
pub fn bounds(text: &Text, fonts: &Fonts) -> Option<Bounds> {
    bounds_at(text, fonts, text.px(), text.max_width)
}

/// [`bounds`] rasterized at `px` rather than the text's own
/// `pixels_per_em` - what a screen-space text uses to land 1:1 on pixels —
/// and wrapped at `width` entity units, or not at all.
pub fn bounds_at(text: &Text, fonts: &Fonts, px: u32, width: Option<f32>) -> Option<Bounds> {
    let layout = layout_at(text, fonts, px, width)?;
    Some(bounds_of(text, &layout, px))
}

fn bounds_of(text: &Text, layout: &TextLayout, px: u32) -> Bounds {
    let s = scale(text, px);
    let (mut left, mut right) = (f32::INFINITY, f32::NEG_INFINITY);
    for &width in &layout.line_widths {
        let start = offset(text, width);
        left = left.min(start);
        right = right.max(start + width);
    }
    Bounds {
        min: Vec2::new(left, -layout.last_baseline() + layout.descent) * s,
        max: Vec2::new(right, layout.ascent) * s,
    }
}

/// Where a caret can stand on the first line, as x in the entity's space,
/// the alignment applied: one stop before each character of the content
/// and one after the last, in the content's order — `chars().count() + 1`
/// of them for one line. Right-to-left text puts a character's stop on its
/// right. `None` with no usable font. Needs no glyph rasterized: the
/// shaper's advances place them.
pub fn caret_stops_at(text: &Text, fonts: &Fonts, px: u32) -> Option<Vec<f32>> {
    let layout = layout_at(text, fonts, px, None)?;
    let s = scale(text, px);
    let width = layout.line_widths.first().copied().unwrap_or(0.0);
    let start = offset(text, width);
    Some(
        layout
            .carets
            .iter()
            .filter(|caret| caret.line == 0)
            .map(|caret| (start + caret.x) * s)
            .collect(),
    )
}

/// Every glyph quad a text draws, and the texture they sample.
///
/// Empty when the font has no face, and missing the glyphs
/// [`prepare`](super::prepare) has not rasterized — a text added since the
/// last prepare appears a frame later, never with wrong UVs. The texture is
/// `None` until the atlas's first upload; the quads are computed anyway, so
/// their placement is testable without a GPU.
pub fn glyphs(text: &Text, fonts: &Fonts) -> (Vec<GlyphQuad>, Option<Handle<Texture>>) {
    glyphs_at(text, fonts, text.px(), text.max_width)
}

/// [`glyphs`] rasterized at `px` and wrapped at `width`; see [`bounds_at`].
pub fn glyphs_at(
    text: &Text,
    fonts: &Fonts,
    px: u32,
    width: Option<f32>,
) -> (Vec<GlyphQuad>, Option<Handle<Texture>>) {
    let Some(layout) = layout_at(text, fonts, px, width) else {
        return (Vec::new(), None);
    };
    let s = scale(text, px);
    let atlas = fonts.atlas();
    let quads = layout
        .glyphs
        .iter()
        .filter_map(|placed| {
            let glyph = atlas.get(placed.key)?;
            let rect = glyph.rect?;
            let m = glyph.metrics;
            let start = offset(text, layout.line_widths[placed.line]);
            let baseline = -layout.baselines[placed.line] + placed.rise;
            let left = (start + placed.pen + m.xmin as f32) * s;
            let bottom = (baseline + m.ymin as f32) * s;
            let right = left + m.width as f32 * s;
            let top = bottom + m.height as f32 * s;
            Some(GlyphQuad {
                corners: [
                    Vec2::new(left, top),
                    Vec2::new(left, bottom),
                    Vec2::new(right, bottom),
                    Vec2::new(right, top),
                ],
                uvs: atlas.uvs(rect),
                color: text.color,
            })
        })
        .collect();
    (quads, fonts.texture())
}

/// The layout [`glyphs_at`] would draw from, for a caller that rasterizes
/// it: what [`prepare`](super::prepare) does for every text.
pub(crate) fn wrapped(text: &Text, fonts: &Fonts, px: u32) -> Option<Arc<TextLayout>> {
    layout_at(text, fonts, px, text.max_width)
}
