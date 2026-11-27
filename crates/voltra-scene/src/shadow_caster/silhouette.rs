//! A caster shaped by its sprite: the outline of the sprite's opaque texels,
//! traced from its texture (see [`voltra_assets::Silhouettes`]) for the
//! frame it draws now, carried onto its quad.

use voltra_assets::silhouette::Rect;
use voltra_assets::{AssetPath, Silhouettes};
use voltra_ecs::{Entity, World};
use voltra_render::glam::{Mat2, Mat3, Vec2};

use super::{CasterShape, ShadowCaster2D};
use crate::sprite::quad::{frame_missing, quad, Quad};
use crate::sprite::sheets::Sheets;
use crate::Sprite2D;

/// What a sprite-shaped caster needs to find its outline: the sheets its
/// quad is resolved against, and the traced outlines.
#[derive(Clone, Copy)]
pub struct SpriteShapes<'a> {
    /// The sheets a sprite's quad is resolved against.
    pub sheets: Sheets<'a>,
    /// The traced outlines.
    pub silhouettes: &'a Silhouettes,
}

/// The texture and the rectangle of it `sprite` draws now — its frame,
/// region or the whole texture — under which its outline is traced; `None`
/// without a texture, one whose size is not known yet, or a frame that is
/// not there. One function for the tracing and the reading, so both name the
/// same rectangle.
pub fn silhouette_key<'s>(
    sprite: &'s Sprite2D,
    sheets: Sheets<'_>,
) -> Option<(&'s AssetPath, Rect, Quad)> {
    let path = sprite.texture.as_ref()?;
    if frame_missing(sprite, sheets) {
        return None;
    }
    let drawn = quad(sprite, sheets);
    let size = sheets.size(drawn.texture)?.as_vec2();
    let (lo, hi) = drawn.uvs.iter().fold(
        (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
        |(lo, hi), uv| (lo.min(Vec2::from(*uv)), hi.max(Vec2::from(*uv))),
    );
    let (min, max) = ((lo * size).round(), (hi * size).round());
    let rect = [
        min.x as u32,
        min.y as u32,
        (max.x - min.x) as u32,
        (max.y - min.y) as u32,
    ];
    Some((path, rect, drawn))
}

/// Traces the frame every sprite-shaped caster draws now, if it has not
/// been. Called before the frame's lighting is gathered.
pub fn prepare(world: &World, sheets: Sheets<'_>, silhouettes: &mut Silhouettes) {
    for (entity, caster) in world.query::<ShadowCaster2D>() {
        let CasterShape::Sprite { alpha } = caster.shape else {
            continue;
        };
        if let Some((path, rect, _)) = world
            .get::<Sprite2D>(entity)
            .and_then(|sprite| silhouette_key(sprite, sheets))
        {
            silhouettes.ensure(path, rect, alpha);
        }
    }
}

/// `entity`'s sprite outline in the world, through `matrix`; nothing
/// without a sprite, a texture or a traced outline.
pub(super) fn sprite_outlines(
    world: &World,
    entity: Entity,
    matrix: Mat3,
    alpha: u8,
    shapes: SpriteShapes<'_>,
) -> Vec<Vec<Vec2>> {
    let Some((path, rect, drawn)) = world
        .get::<Sprite2D>(entity)
        .and_then(|sprite| silhouette_key(sprite, shapes.sheets))
    else {
        return Vec::new();
    };
    let Some(outlines) = shapes.silhouettes.get(path, rect, alpha) else {
        return Vec::new();
    };
    let Some(to_local) = uv_to_local(&drawn) else {
        return Vec::new();
    };
    // The outline is in 0..1 across the rectangle; the quad's UVs say where
    // the rectangle's corners are, flips and all.
    let (lo, hi) = drawn.uvs.iter().fold(
        (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
        |(lo, hi), uv| (lo.min(Vec2::from(*uv)), hi.max(Vec2::from(*uv))),
    );
    outlines
        .iter()
        .map(|ring| {
            ring.iter()
                .map(|p| matrix.transform_point2(to_local(lo + *p * (hi - lo))))
                .collect()
        })
        .collect()
}

/// `entity`'s sprite outline in its own space — where its collider's points
/// are — traced first if it has not been; nothing without a sprite, a
/// texture, or opaque texels. What a collider fitted to its sprite is the
/// convex hull of.
pub fn sprite_silhouette(
    world: &World,
    entity: Entity,
    alpha: u8,
    sheets: Sheets<'_>,
    silhouettes: &mut Silhouettes,
) -> Vec<Vec<Vec2>> {
    let Some((path, rect, _)) = world
        .get::<Sprite2D>(entity)
        .and_then(|sprite| silhouette_key(sprite, sheets))
    else {
        return Vec::new();
    };
    silhouettes.ensure(path, rect, alpha);
    sprite_outlines(
        world,
        entity,
        Mat3::IDENTITY,
        alpha,
        SpriteShapes {
            sheets,
            silhouettes,
        },
    )
}

/// The affine map from a texture UV to where it lands on `drawn`'s quad, in
/// the sprite's space, read off three of its corners; `None` for a quad
/// with no area.
fn uv_to_local(drawn: &Quad) -> Option<impl Fn(Vec2) -> Vec2> {
    let corners = drawn.corners();
    let uv = |i: usize| Vec2::from(drawn.uvs[i]);
    // Top-left, bottom-left and top-right.
    let (a, b, c) = (corners[0], corners[1], corners[3]);
    let (ua, ub, uc) = (uv(0), uv(1), uv(3));
    let across = Mat2::from_cols(ub - ua, uc - ua);
    // Only an exact zero: a small frame on a large sheet has a tiny
    // determinant (its share of the sheet, squared) and real area.
    let determinant = across.determinant();
    if determinant == 0.0 || !determinant.is_finite() {
        return None;
    }
    let map = Mat2::from_cols(b - a, c - a) * across.inverse();
    Some(move |at: Vec2| a + map * (at - ua))
}

#[cfg(test)]
#[path = "silhouette_tests.rs"]
mod tests;
