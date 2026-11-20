//! A grid of tiles cut from one atlas: a layer of a level.
//!
//! **One entity per layer, chunks inside it.** Godot 4 split its `TileMap`
//! into one `TileMapLayer` node per layer and keeps rendering quadrants
//! inside each; Unity's `Tilemap` is one component per layer the same way.
//! One entity per *tile* (bevy_ecs_tilemap's shape) makes a 100×100 level ten
//! thousand entities the hierarchy, the save and every query walk for no
//! reason; one entity per *chunk* makes painting across a chunk border an
//! entity spawn. A layer is the thing an author thinks about — "the ground",
//! "the decoration in front" — so a layer is the entity, and it has a
//! `Transform`, a `sort_order` and a place in the hierarchy like a sprite.
//!
//! **Sparse chunks, dense cells.** A map is the chunks that hold a tile, each
//! a fixed [`CHUNK`]×[`CHUNK`] square. Painting off any edge, at negative
//! coordinates, or on an island far from the rest costs one chunk, not a
//! resize of the whole map — LDtk's and Tiled's infinite maps both settle
//! here.
//!
//! **The tileset is an atlas.** Stage 22's slicing is exactly what a tileset
//! is: frames cut from one sheet. A cell stores a frame index into it.

mod chunk;
pub mod mesh;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use voltra_assets::{AssetPath, Atlas, Atlases, Handle, Textures};
use voltra_render::glam::{IVec2, Vec2};
use voltra_render::Texture;

pub use chunk::CHUNK;
use chunk::{cell_of, locate, Chunk, ChunkKey};

/// One cell's contents: which frame of the atlas, and which way round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Tile {
    pub frame: u32,
    pub flip_x: bool,
    pub flip_y: bool,
}

impl Tile {
    /// The highest frame a cell can name: the top two bits of a stored cell
    /// are its flips, and `0` is an empty cell.
    pub const MAX_FRAME: u32 = (1 << 30) - 2;

    pub fn new(frame: u32) -> Self {
        Self {
            frame,
            ..Self::default()
        }
    }
}

/// A layer of tiles, drawn from one atlas's frames.
///
/// Cell `(x, y)` covers `x..x+1` by `y..y+1` tiles in the entity's own space,
/// so cell `(0, 0)` has its lower-left corner on the entity's position — the
/// rule Godot, Unity and Tiled share, and the one that makes a map line up
/// with integer coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tilemap {
    /// How big one cell is, in the entity's units.
    pub tile_size: Vec2,
    /// Multiplied into every tile, as a sprite's colour is.
    pub color: [f32; 4],
    /// Where the layer draws among sprites and other layers. The same key a
    /// sprite sorts by, so a layer goes in front of or behind them by number.
    pub sort_order: i32,
    /// The sheet the atlas cuts.
    pub texture: Option<AssetPath>,
    #[serde(skip)]
    pub texture_handle: Option<Handle<Texture>>,
    /// The slicing whose frames the cells name.
    pub atlas: Option<AssetPath>,
    #[serde(skip)]
    pub atlas_handle: Option<Handle<Atlas>>,
    #[serde(with = "chunk::layout")]
    chunks: BTreeMap<ChunkKey, Chunk>,
}

impl Default for Tilemap {
    fn default() -> Self {
        Self {
            tile_size: Vec2::ONE,
            color: [1.0, 1.0, 1.0, 1.0],
            sort_order: 0,
            texture: None,
            texture_handle: None,
            atlas: None,
            atlas_handle: None,
            chunks: BTreeMap::new(),
        }
    }
}

impl Tilemap {
    /// An empty layer whose cells are `tile_size` across.
    pub fn new(tile_size: Vec2) -> Self {
        Self {
            tile_size,
            ..Self::default()
        }
    }

    /// What is at `cell`, if anything.
    pub fn get(&self, cell: IVec2) -> Option<Tile> {
        let (key, index) = locate(cell);
        self.chunks.get(&key)?.get(index)
    }

    /// Puts `tile` at `cell` — or empties it, for `None` — and returns what
    /// was there.
    ///
    /// A frame above [`Tile::MAX_FRAME`] cannot be stored and leaves the cell
    /// as it was; it is logged rather than wrapped into a different tile. The
    /// chunk a cell needs is made on the way in, and dropped once its last
    /// tile is erased, so an erased map saves as nothing.
    pub fn set(&mut self, cell: IVec2, tile: Option<Tile>) -> Option<Tile> {
        if let Some(tile) = tile.filter(|tile| tile.frame > Tile::MAX_FRAME) {
            log::warn!(
                "tile frame {} is beyond the {} a cell can hold; cell left as it was",
                tile.frame,
                Tile::MAX_FRAME
            );
            return self.get(cell);
        }

        let (key, index) = locate(cell);
        let Some(tile) = tile else {
            let chunk = self.chunks.get_mut(&key)?;
            let previous = chunk.set(index, None);
            if chunk.is_empty() {
                self.chunks.remove(&key);
            }
            return previous;
        };
        self.chunks
            .entry(key)
            .or_insert_with(Chunk::empty)
            .set(index, Some(tile))
    }

    /// Every tile, with its cell, chunk by chunk.
    pub fn tiles(&self) -> impl Iterator<Item = (IVec2, Tile)> + '_ {
        self.chunks.iter().flat_map(|(&key, chunk)| {
            chunk
                .tiles()
                .map(move |(index, tile)| (cell_of(key, index), tile))
        })
    }

    /// How many cells hold a tile.
    pub fn len(&self) -> usize {
        self.chunks.values().map(Chunk::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Empties every cell.
    pub fn clear(&mut self) {
        self.chunks.clear();
    }

    /// The lowest and highest cell holding a tile, or `None` for an empty map.
    pub fn bounds(&self) -> Option<(IVec2, IVec2)> {
        self.tiles().fold(None, |bounds, (cell, _)| match bounds {
            None => Some((cell, cell)),
            Some((min, max)) => Some((min.min(cell), max.max(cell))),
        })
    }

    /// The lower-left corner of `cell`, in the entity's own space.
    pub fn cell_origin(&self, cell: IVec2) -> Vec2 {
        cell.as_vec2() * self.tile_size
    }

    /// Sets or clears the sheet path and refreshes its handle, as
    /// [`Sprite::set_texture`](crate::Sprite::set_texture) does.
    pub fn set_texture(
        &mut self,
        path: Option<AssetPath>,
        textures: &mut Textures,
        device: &voltra_render::wgpu::Device,
        queue: &voltra_render::wgpu::Queue,
    ) {
        self.texture_handle = path.as_ref().map(|path| textures.load(device, queue, path));
        self.texture = path;
    }

    /// Sets or clears the atlas path and refreshes its handle, as
    /// [`Sprite::set_atlas`](crate::Sprite::set_atlas) does.
    pub fn set_atlas(&mut self, path: Option<AssetPath>, atlases: &mut Atlases) {
        self.atlas_handle = path.as_ref().map(|path| atlases.load(path));
        self.atlas = path;
    }
}

#[cfg(test)]
mod mesh_tests;
#[cfg(test)]
mod tests;
