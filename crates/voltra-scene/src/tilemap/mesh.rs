//! A tilemap layer as quads in the sprite batch.
//!
//! The same vertices, the same ranges and the same draw call a sprite makes —
//! nothing new reaches the renderer. Every tile of a layer binds the one
//! sheet (and its normal map), so a layer is one range however many tiles it
//! holds, unless a missing frame drops a placeholder into the middle of it.

use voltra_render::glam::{IVec2, Mat3, Vec2};

use super::cache::{LayerCache, LayerStamp};
use super::chunk::{cell_of, ChunkKey, CHUNK};
use super::{Tile, Tilemap};
use crate::batch::{SpriteBatch, Surface};
use crate::sprite::quad::{flipped, WHOLE};
use crate::sprite::sheets::Sheets;

/// Appends every tile of `map`, placed by `matrix`, to `batch`, with nothing
/// kept for the next frame — a pick, a test, a one-off batch.
pub fn push(batch: &mut SpriteBatch, matrix: Mat3, map: &Tilemap, sheets: Sheets<'_>) {
    let material = batch.material(map.material.as_ref(), sheets);
    for (&key, chunk) in &map.chunks {
        if in_view(map, matrix, key, sheets).is_none() {
            continue;
        }
        for (index, tile) in chunk.tiles() {
            let (uvs, texture) = sample(map, tile, sheets);
            let corners = corners_of(map, cell_of(key, index));
            batch.push_quad_on(
                matrix,
                corners,
                uvs,
                map.color,
                surface(map, texture, material),
            );
        }
    }
}

/// Appends every tile of `map`, placed by `matrix`, to `batch`, through
/// `cache`: a chunk drawn last frame from the same cells is copied rather
/// than rebuilt. The same quads [`push`] writes. See [`cache`](super::cache).
pub(crate) fn push_cached(
    batch: &mut SpriteBatch,
    matrix: Mat3,
    map: &Tilemap,
    sheets: Sheets<'_>,
    cache: &mut LayerCache,
) {
    let stamp = LayerStamp::of(map, sheets);
    let material = batch.material(map.material.as_ref(), sheets);
    for (&key, chunk) in &map.chunks {
        if let Some(inside) = in_view(map, matrix, key, sheets) {
            let at = super::cache::Place { inside, material };
            cache.push_chunk(batch, matrix, map, key, chunk, sheets, &stamp, at);
        }
    }
}

/// Whether the chunk at `key` is seen at all — `None` skips it without
/// visiting its cells, which is what makes a level of tens of thousands of
/// tiles cost the screenful in view rather than all of it — and if it is,
/// whether it is seen whole, so none of its quads can be culled.
fn in_view(map: &Tilemap, matrix: Mat3, key: ChunkKey, sheets: Sheets<'_>) -> Option<bool> {
    let first = map.cell_origin(IVec2::new(key.0 * CHUNK, key.1 * CHUNK));
    let last = map.cell_origin(IVec2::new((key.0 + 1) * CHUNK, (key.1 + 1) * CHUNK));
    let corners = [
        first,
        Vec2::new(first.x, last.y),
        last,
        Vec2::new(last.x, first.y),
    ]
    .map(|corner| matrix.transform_point2(corner));
    let min = corners.iter().copied().fold(Vec2::INFINITY, Vec2::min);
    let max = corners.iter().copied().fold(Vec2::NEG_INFINITY, Vec2::max);
    if !sheets.sees(min, max) {
        return None;
    }
    Some(
        min.is_finite()
            && max.is_finite()
            && sheets.visible.is_none_or(|(view_min, view_max)| {
                min.cmpge(view_min).all() && max.cmple(view_max).all()
            }),
    )
}

/// One tile's corners in the layer's space: top-left, bottom-left,
/// bottom-right, top-right — the order a sprite's corners and UVs are in.
pub(super) fn corners_of(map: &Tilemap, cell: IVec2) -> [Vec2; 4] {
    let origin = map.cell_origin(cell);
    let size = map.tile_size;
    [
        Vec2::new(origin.x, origin.y + size.y),
        origin,
        Vec2::new(origin.x + size.x, origin.y),
        origin + size,
    ]
}

/// What a tile drawing `texture` is drawn with: the layer's normal map over
/// its sheet — not over the placeholder a missing frame draws, which is flat
/// — and the layer's light layers.
pub(super) fn surface(
    map: &Tilemap,
    texture: Option<voltra_assets::Handle<voltra_render::Texture>>,
    material: Option<u32>,
) -> Surface {
    Surface {
        texture,
        normal: map.normal_handle.filter(|_| texture == map.texture_handle),
        light_layers: map.light_layers,
        material,
    }
}

/// The UVs and texture one tile draws with.
///
/// Missing in the same way a sprite's frame is missing — the layer names an
/// atlas that is not loaded, or a frame it does not have — draws the
/// placeholder checker over the whole cell. A layer with no atlas at all
/// draws its whole texture in every cell, which is what a sprite with no
/// atlas does.
pub(super) fn sample(
    map: &Tilemap,
    tile: Tile,
    sheets: Sheets<'_>,
) -> (
    [[f32; 2]; 4],
    Option<voltra_assets::Handle<voltra_render::Texture>>,
) {
    let found = if map.atlas.is_some() {
        sheets
            .atlas(map.atlas_handle)
            .and_then(|atlas| {
                atlas
                    .frame(map.shown_frame(tile.frame, sheets.time))?
                    .uv(atlas.sheet())
            })
            .map(|uv| (uv, map.texture_handle))
    } else {
        Some((WHOLE, map.texture_handle))
    };
    let ((min, max), texture) =
        found.unwrap_or((WHOLE, sheets.placeholder().or(map.texture_handle)));

    let (u0, u1) = flipped(tile.flip_x, min[0], max[0]);
    let (v0, v1) = flipped(tile.flip_y, min[1], max[1]);
    ([[u0, v0], [u0, v1], [u1, v1], [u1, v0]], texture)
}
