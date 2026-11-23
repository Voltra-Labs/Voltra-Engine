//! Screen-space elements as quads in the target's pixels.

use voltra_ecs::{Entity, World};
use voltra_render::glam::{Mat3, Vec2};

use super::layout::UiLayout;
use super::rect::Rect;
use super::slice::{Image, UiSlice};
use super::UiRect;
use crate::batch::SpriteBatch;
use crate::sprite::quad::{quad, texels};
use crate::sprite::sheets::Sheets;
use crate::sprite::Sprite;
use crate::text::{shape, ui_px, Text};

mod clip;
mod field;

/// Draw order, then sprite (0) before text (1) on one entity.
type ElementKey = (i32, u32, u32, u8);

/// What one screen-space element draws.
enum Element<'a> {
    /// With the 9-slice it is drawn as, if it has one.
    Sprite(&'a Sprite, Option<&'a UiSlice>),
    Text(&'a Text),
}

/// Every element with a [`UiRect`] and a `Sprite` or `Text`, as a batch in
/// physical pixels for [`screen_projection`](voltra_render::screen_projection).
///
/// `layout` is in points; `scale` is physical pixels per point — the
/// window's scale factor — so a HUD keeps its size on a high-density
/// display. An entity carrying both draws its sprite under its text, as a
/// button's face and label would.
pub fn batch(world: &World, sheets: Sheets<'_>, layout: &UiLayout, scale: f32) -> SpriteBatch {
    let mut elements: Vec<(ElementKey, Entity, Element<'_>)> = Vec::new();
    for (entity, _) in world.query::<UiRect>() {
        if let Some(sprite) = world.get::<Sprite>(entity) {
            let (order, depth, index) = layout.draw_key(entity, sprite.sort_order);
            let slice = world.get::<UiSlice>(entity);
            elements.push((
                (order, depth, index, 0),
                entity,
                Element::Sprite(sprite, slice),
            ));
        }
        if let Some(text) = world.get::<Text>(entity) {
            let (order, depth, index) = layout.draw_key(entity, text.sort_order);
            elements.push(((order, depth, index, 1), entity, Element::Text(text)));
        }
    }
    // The last key component keeps an entity's sprite under its own text;
    // every key is unique, so the unstable sort loses nothing.
    elements.sort_unstable_by_key(|(key, _, _)| *key);

    let mut batch = SpriteBatch::default();
    for (_, entity, element) in elements {
        let Some(rect) = layout.rect(entity) else {
            continue;
        };
        // The mask is in pixels, as the quads are.
        let mask = layout
            .clip(entity)
            .map(|clip| Rect::new(clip.min * scale, clip.max * scale));
        let target = Target {
            batch: &mut batch,
            mask,
        };
        match element {
            Element::Sprite(sprite, slice) => {
                push_sprite(target, rect, sprite, slice, sheets, scale)
            }
            Element::Text(text) => match world.get::<super::UiTextField>(entity) {
                Some(field) => field::push(target, rect, text, field, sheets, scale),
                None => push_text(target, rect, text, sheets, scale),
            },
        }
    }
    batch
}

/// Where quads go: the batch, cut to the element's mask.
struct Target<'a> {
    batch: &'a mut SpriteBatch,
    mask: Option<Rect>,
}

impl Target<'_> {
    fn push(
        &mut self,
        corners: [Vec2; 4],
        uvs: [[f32; 2]; 4],
        color: [f32; 4],
        texture: Option<voltra_assets::Handle<voltra_render::Texture>>,
    ) {
        clip::push(self.batch, corners, uvs, color, texture, self.mask);
    }
}

/// A sprite stretched over its rect, with its frame, region and flips — or,
/// with a 9-slice, as its nine pieces.
///
/// A 9-slice needs the frame's size in texels to know where its borders
/// fall; without it — no texture store, a frame that is not there — the
/// sprite is stretched whole, as it would be with no slice at all.
fn push_sprite(
    mut target: Target<'_>,
    rect: Rect,
    sprite: &Sprite,
    slice: Option<&UiSlice>,
    sheets: Sheets<'_>,
    scale: f32,
) {
    let (min, max) = (rect.min * scale, rect.max * scale);
    if max.x <= min.x || max.y <= min.y {
        return;
    }
    let quad = quad(sprite, sheets);
    if let (Some(slice), Some(texels)) = (slice, texels(sprite, sheets)) {
        let image = Image {
            texels,
            uvs: quad.uvs,
            flip_x: sprite.flip_x,
            flip_y: sprite.flip_y,
        };
        for piece in slice.pieces(Rect::new(min, max), image, scale) {
            target.push(piece.corners, piece.uvs, sprite.color, quad.texture);
        }
        return;
    }
    // The batch's corner order — top-left, bottom-left, bottom-right,
    // top-right — with y already pointing down.
    let corners = [
        Vec2::new(min.x, min.y),
        Vec2::new(min.x, max.y),
        Vec2::new(max.x, max.y),
        Vec2::new(max.x, min.y),
    ];
    target.push(corners, quad.uvs, sprite.color, quad.texture);
}

/// A text laid out inside its rect: the first baseline one ascent below the
/// top, the lines aligned to the rect's left edge, middle or right edge.
///
/// Rasterized at its on-screen size and placed on a whole pixel, so glyph
/// texels map 1:1 onto the target.
fn push_text(mut target: Target<'_>, rect: Rect, text: &Text, sheets: Sheets<'_>, scale: f32) {
    let Some(fonts) = sheets.fonts else {
        return;
    };
    let px = ui_px(text, scale);
    let Some(bounds) = shape::bounds_at(text, fonts, px) else {
        return;
    };
    let (quads, Some(texture)) = shape::glyphs_at(text, fonts, px) else {
        return;
    };
    let x = super::text_field::view::aligned_x(rect, text.align);
    // `bounds.max.y` is the ascent, in the text's own units — points here.
    let origin = (Vec2::new(x, rect.min.y + bounds.max.y) * scale).round();
    // Glyph quads come y-up in points; the target is y-down in pixels.
    let matrix = Mat3::from_translation(origin) * Mat3::from_scale(Vec2::new(scale, -scale));
    for glyph in quads {
        let corners = glyph.corners.map(|c| matrix.transform_point2(c));
        target.push(corners, glyph.uvs, text.color, Some(texture));
    }
}

#[cfg(test)]
mod tests;
