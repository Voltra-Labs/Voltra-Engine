//! A tilemap layer as quads in the sprite batch.
//!
//! The same vertices, the same ranges and the same draw call a sprite makes —
//! nothing new reaches the renderer. Every tile of a layer binds the one
//! sheet, so a layer is one range however many tiles it holds, unless a
//! missing frame drops a placeholder into the middle of it.

use voltra_render::glam::{IVec2, Mat3, Vec2};

use super::chunk::{cell_of, CHUNK};
use super::{Tile, Tilemap};
use crate::batch::SpriteBatch;
use crate::sprite::quad::{flipped, WHOLE};
use crate::sprite::sheets::Sheets;

/// Appends every tile of `map`, placed by `matrix`, to `batch`.
///
/// Rebuilt every frame with the sprites, like the sprites. A layer is a few
/// thousand quads, and the CPU walk is what the rest of the batch already
/// costs once chunks out of view are skipped, which is what a level's size
/// stops mattering for; caching a mesh per chunk is the step after.
pub fn push(batch: &mut SpriteBatch, matrix: Mat3, map: &Tilemap, sheets: Sheets<'_>) {
    for (&key, chunk) in &map.chunks {
        // A whole chunk off screen is skipped without visiting its cells:
        // what makes a level of tens of thousands of tiles cost the screenful
        // in view rather than all of it.
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
            continue;
        }
        for (index, tile) in chunk.tiles() {
            push_tile(batch, matrix, map, cell_of(key, index), tile, sheets);
        }
    }
}

/// One tile's quad.
fn push_tile(
    batch: &mut SpriteBatch,
    matrix: Mat3,
    map: &Tilemap,
    cell: IVec2,
    tile: Tile,
    sheets: Sheets<'_>,
) {
    let origin = map.cell_origin(cell);
    let size = map.tile_size;
    // Top-left, bottom-left, bottom-right, top-right: the order a sprite's
    // corners and UVs are in.
    let corners = [
        Vec2::new(origin.x, origin.y + size.y),
        origin,
        Vec2::new(origin.x + size.x, origin.y),
        origin + size,
    ];
    let (uvs, texture) = sample(map, tile, sheets);
    batch.push_quad(matrix, corners, uvs, map.color, texture);
}

/// The UVs and texture one tile draws with.
///
/// Missing in the same way a sprite's frame is missing — the layer names an
/// atlas that is not loaded, or a frame it does not have — draws the
/// placeholder checker over the whole cell. A layer with no atlas at all
/// draws its whole texture in every cell, which is what a sprite with no
/// atlas does.
fn sample(
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
            .and_then(|atlas| atlas.frame(tile.frame)?.uv(atlas.sheet()))
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
