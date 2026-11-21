//! A tilemap layer's quads kept from one frame to the next, chunk by chunk.
//!
//! Rebuilding a layer means resolving every tile's frame through its atlas,
//! every frame; with a whole level in view that is milliseconds. Godot's
//! `TileMapLayer` keeps a canvas item per quadrant and rebuilds a quadrant
//! when it is dirty; this is the same split at the same grain (a chunk is
//! the unit cells are stored and culled in already), with a stamp of what
//! the quads were built from in place of a dirty flag — see [`Revision`].
//!
//! Two levels, because they go stale for different reasons:
//!
//! - **Local quads** — corners in the layer's space, UVs, texture — go stale
//!   when the cells, the tile size, the sheet or atlas, the atlas's contents,
//!   or an animated tile's current frame change.
//! - **World vertices** — the local quads through the layer's matrix, in its
//!   colour — go stale when the layer moves or is recoloured. A still layer
//!   wholly in view is a copy; a moving one re-transforms, but never
//!   re-resolves a frame.
//!
//! A chunk straddling the edge of the view is pushed quad by quad from its
//! local quads, so each quad is culled exactly as an uncached one would be:
//! the cache changes the cost of a batch, never its contents.
//!
//! [`Revision`]: super::Revision

use std::hash::{Hash, Hasher};

use voltra_assets::Handle;
use voltra_ecs::hash::FastMap;
use voltra_ecs::Entity;
use voltra_render::glam::{IVec2, Mat3, Vec2};
use voltra_render::{Texture, Vertex};

use super::chunk::{cell_of, Chunk, ChunkKey};
use super::mesh::{corners_of, sample};
use super::Tilemap;
use crate::batch::SpriteBatch;
use crate::sprite::sheets::Sheets;

/// Every tilemap layer's cached chunks, owned by whoever builds batches
/// frame after frame — the app's draw path.
///
/// Entries not drawn in a build are dropped at the end of it: a despawned
/// layer, an erased chunk, a chunk scrolled out of view. Memory follows what
/// is on screen, not what was ever on screen; a chunk coming back into view
/// costs one rebuild.
#[derive(Debug, Default)]
pub struct TileMeshCache {
    /// Keyed by the layer and which copy of it — a repeating parallax layer
    /// is drawn several times, each through its own matrix.
    layers: FastMap<(Entity, usize), LayerCache>,
    build: u64,
}

impl TileMeshCache {
    /// Starts a batch build.
    pub(crate) fn begin(&mut self) {
        self.build = self.build.wrapping_add(1);
    }

    /// The cache of `copy` of the layer `entity`.
    pub(crate) fn layer(&mut self, entity: Entity, copy: usize) -> &mut LayerCache {
        let build = self.build;
        let layer = self.layers.entry((entity, copy)).or_default();
        layer.build = build;
        layer
    }

    /// Ends a batch build, dropping whatever it did not draw.
    pub(crate) fn finish(&mut self) {
        let build = self.build;
        self.layers.retain(|_, layer| layer.build == build);
        for layer in self.layers.values_mut() {
            layer.chunks.retain(|_, chunk| chunk.build == build);
        }
    }

    /// Chunks held, across every layer.
    pub fn chunk_count(&self) -> usize {
        self.layers.values().map(|layer| layer.chunks.len()).sum()
    }
}

/// One copy of one layer.
#[derive(Debug, Default)]
pub(crate) struct LayerCache {
    chunks: FastMap<ChunkKey, ChunkMesh>,
    build: u64,
}

/// What a chunk's local quads were built from.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Stamp {
    revision: u64,
    tile_size: Vec2,
    sheet: Option<Handle<Texture>>,
    atlas: Option<Handle<voltra_assets::Atlas>>,
    uses_atlas: bool,
    atlases: u64,
    placeholder: Option<Handle<Texture>>,
    /// The layer's animations, as defined.
    animations: u64,
    /// The frame each animation is on, for a chunk holding an animated
    /// tile; zero for one that holds none, which no clock changes.
    phase: u64,
}

#[derive(Debug, Default)]
struct ChunkMesh {
    stamp: Option<Stamp>,
    /// Whether any tile names a frame the layer animates.
    animated: bool,
    quads: Vec<LocalQuad>,
    world: Option<WorldMesh>,
    build: u64,
}

#[derive(Debug, Clone, Copy)]
struct LocalQuad {
    corners: [Vec2; 4],
    uvs: [[f32; 2]; 4],
    texture: Option<Handle<Texture>>,
}

#[derive(Debug)]
struct WorldMesh {
    matrix: Mat3,
    color: [f32; 4],
    vertices: Vec<Vertex>,
    /// Two triangles per quad, counted from the chunk's first vertex.
    indices: Vec<u32>,
    /// Consecutive quads sharing a texture, in order.
    runs: Vec<(Option<Handle<Texture>>, u32)>,
}

/// What every chunk of `map` is stamped with this frame, before its phase.
pub(super) struct LayerStamp {
    stamp: Stamp,
    phase: u64,
}

impl LayerStamp {
    pub(super) fn of(map: &Tilemap, sheets: Sheets<'_>) -> Self {
        let mut definitions = voltra_ecs::hash::FastHasher::default();
        let mut phase = voltra_ecs::hash::FastHasher::default();
        for (&frame, animation) in &map.animations {
            frame.hash(&mut definitions);
            animation.frames.hash(&mut definitions);
            animation.frame_time.to_bits().hash(&mut definitions);
            map.shown_frame(frame, sheets.time).hash(&mut phase);
        }
        Self {
            stamp: Stamp {
                revision: map.revision().get(),
                tile_size: map.tile_size,
                sheet: map.texture_handle,
                atlas: map.atlas_handle,
                uses_atlas: map.atlas.is_some(),
                atlases: sheets.atlases.map_or(0, |atlases| atlases.generation()),
                placeholder: sheets.placeholder(),
                animations: definitions.finish(),
                phase: 0,
            },
            // Never zero, so an animated chunk is told apart from a still one.
            phase: phase.finish() | 1,
        }
    }

    fn for_chunk(&self, animated: bool) -> Stamp {
        Stamp {
            phase: if animated { self.phase } else { 0 },
            ..self.stamp
        }
    }
}

impl LayerCache {
    /// Appends `chunk`'s quads, placed by `matrix`, to `batch`, building or
    /// reusing what is cached for it. `inside` says the whole chunk is in
    /// view, which is what lets its quads skip the per-quad cull.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn push_chunk(
        &mut self,
        batch: &mut SpriteBatch,
        matrix: Mat3,
        map: &Tilemap,
        key: ChunkKey,
        chunk: &Chunk,
        sheets: Sheets<'_>,
        stamp: &LayerStamp,
        inside: bool,
    ) {
        let build = self.build;
        let entry = self.chunks.entry(key).or_default();
        entry.build = build;
        if entry.stamp != Some(stamp.for_chunk(entry.animated)) {
            entry.rebuild(map, key, chunk, sheets);
            entry.stamp = Some(stamp.for_chunk(entry.animated));
        }
        if inside {
            entry.append_world(batch, matrix, map.color);
        } else {
            for quad in &entry.quads {
                batch.push_quad(matrix, quad.corners, quad.uvs, map.color, quad.texture);
            }
        }
    }
}

impl ChunkMesh {
    fn rebuild(&mut self, map: &Tilemap, key: ChunkKey, chunk: &Chunk, sheets: Sheets<'_>) {
        self.quads.clear();
        self.world = None;
        self.animated = false;
        for (index, tile) in chunk.tiles() {
            let cell: IVec2 = cell_of(key, index);
            self.animated |= map.animations.contains_key(&tile.frame);
            let (uvs, texture) = sample(map, tile, sheets);
            self.quads.push(LocalQuad {
                corners: corners_of(map, cell),
                uvs,
                texture,
            });
        }
    }

    /// The chunk's quads through `matrix`, as [`SpriteBatch::push_quad`]
    /// would have written them, copied in.
    fn append_world(&mut self, batch: &mut SpriteBatch, matrix: Mat3, color: [f32; 4]) {
        let fresh = self
            .world
            .as_ref()
            .is_some_and(|world| world.matrix == matrix && world.color == color);
        if !fresh {
            self.world = Some(WorldMesh::of(&self.quads, matrix, color));
        }
        let world = self.world.as_ref().expect("made just above");

        let base = batch.vertices.len() as u32;
        batch.vertices.extend_from_slice(&world.vertices);
        let first = batch.indices.len() as u32;
        batch
            .indices
            .extend(world.indices.iter().map(|index| base + index));
        let mut start = first;
        for &(texture, quads) in &world.runs {
            let end = start + quads * crate::batch::QUAD_INDICES.len() as u32;
            batch.extend_ranges(texture, start..end);
            start = end;
        }
    }
}

impl WorldMesh {
    fn of(quads: &[LocalQuad], matrix: Mat3, color: [f32; 4]) -> Self {
        let mut vertices = Vec::with_capacity(quads.len() * crate::batch::CORNERS_PER_QUAD);
        let mut indices = Vec::with_capacity(quads.len() * crate::batch::QUAD_INDICES.len());
        let mut runs: Vec<(Option<Handle<Texture>>, u32)> = Vec::new();
        for quad in quads {
            let base = vertices.len() as u32;
            indices.extend(
                crate::batch::QUAD_INDICES
                    .iter()
                    .map(|offset| base + offset),
            );
            for (corner, uv) in quad.corners.into_iter().zip(quad.uvs) {
                let at = matrix.transform_point2(corner);
                vertices.push(Vertex::new([at.x, at.y], color, uv));
            }
            match runs.last_mut() {
                Some((texture, count)) if *texture == quad.texture => *count += 1,
                _ => runs.push((quad.texture, 1)),
            }
        }
        Self {
            matrix,
            color,
            vertices,
            indices,
            runs,
        }
    }
}
