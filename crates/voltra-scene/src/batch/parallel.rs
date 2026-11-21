//! Sprites placed across the job pool before the batch is assembled.
//!
//! A sprite's quad — its frame looked up, its corners through its world
//! matrix, culled against the view — depends on nothing but the sprite, its
//! matrix and the stores, so thousands can be placed at once. Assembling
//! the batch stays one pass on the caller: draw order is the order quads are
//! appended in, and the ranges that batch them depend on every quad before.

use voltra_assets::Handle;
use voltra_ecs::Entity;
use voltra_jobs::JobPool;
use voltra_render::{Texture, Vertex};

use super::build::Drawn;
use super::placed;
use crate::hierarchy::WorldTransforms;
use crate::sprite::quad::quad;
use crate::sprite::sheets::Sheets;

/// Fewer sprites than this are placed on the caller: below it, handing out
/// the work costs more than doing it.
pub(super) const PARALLEL_SPRITES: usize = 512;

/// Sprites per job, at the least.
const CHUNK: usize = 256;

/// What a drawn entity's slot holds after the pre-pass.
#[derive(Clone, Copy)]
pub(super) enum Prepared {
    /// Not a sprite: placed during assembly, as before.
    Other,
    /// A sprite wholly out of view.
    Culled,
    Quad([Vertex; 4], Option<Handle<Texture>>),
}

/// Every sprite in `drawn` placed, in `drawn`'s order, across `jobs`.
pub(super) fn prepare(
    jobs: &JobPool,
    drawn: &[(Entity, Drawn<'_>)],
    transforms: &WorldTransforms,
    sheets: Sheets<'_>,
) -> Vec<Prepared> {
    let mut prepared = vec![Prepared::Other; drawn.len()];
    jobs.for_each_chunk_mut(&mut prepared, CHUNK, |start, slots| {
        for (slot, (entity, drawn)) in slots.iter_mut().zip(&drawn[start..]) {
            let Drawn::Sprite(sprite) = drawn else {
                continue;
            };
            let quad = quad(sprite, sheets);
            *slot = match placed(
                sheets.visible,
                transforms.matrix(*entity),
                quad.corners(),
                quad.uvs,
                sprite.color,
            ) {
                Some(vertices) => Prepared::Quad(vertices, quad.texture),
                None => Prepared::Culled,
            };
        }
    });
    prepared
}
