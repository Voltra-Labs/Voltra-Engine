//! A square of cells, stored densely, and how a map of them is written.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use voltra_render::glam::IVec2;

use super::Tile;

/// Cells along one side of a chunk.
///
/// Godot's rendering quadrant defaults to 16, and so does this, for the same
/// trade: big enough that a level is a handful of chunks rather than
/// thousands, small enough that an island of three tiles in the middle of
/// nowhere does not allocate a screenful of empty cells around itself.
pub const CHUNK: i32 = 16;

/// Cells in one chunk.
pub(super) const CELLS: usize = (CHUNK * CHUNK) as usize;

/// Set on a stored cell whose tile is mirrored left to right. Tiled's bit.
const FLIP_X: u32 = 1 << 31;
/// Set on a stored cell whose tile is mirrored top to bottom. Tiled's bit.
const FLIP_Y: u32 = 1 << 30;
/// What is left of a stored cell for the frame, once the flips are off.
const FRAME_BITS: u32 = !(FLIP_X | FLIP_Y);

/// Where a chunk sits, in chunks: a cell's coordinates divided by [`CHUNK`],
/// rounded towards negative infinity so cell `-1` is in chunk `-1`, not `0`.
///
/// A tuple rather than an `IVec2` because it is a map key, and glam's vectors
/// are not `Ord`.
pub(super) type ChunkKey = (i32, i32);

/// The chunk `cell` is in, and its index inside that chunk.
pub(super) fn locate(cell: IVec2) -> (ChunkKey, usize) {
    let chunk = (cell.x.div_euclid(CHUNK), cell.y.div_euclid(CHUNK));
    let local = (cell.x.rem_euclid(CHUNK), cell.y.rem_euclid(CHUNK));
    (chunk, (local.1 * CHUNK + local.0) as usize)
}

/// The cell at `index` of the chunk at `key`: the inverse of [`locate`].
pub(super) fn cell_of(key: ChunkKey, index: usize) -> IVec2 {
    let index = index as i32;
    IVec2::new(key.0 * CHUNK + index % CHUNK, key.1 * CHUNK + index / CHUNK)
}

/// One chunk's cells, row by row from its lowest corner, as stored: `0` for
/// an empty cell, otherwise the frame plus one with the flips in the top two
/// bits — Tiled's encoding, so a reader of one can read the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Chunk {
    cells: Box<[u32; CELLS]>,
    /// How many cells are not empty, so emptying the last one can drop the
    /// chunk without scanning it.
    filled: u16,
}

impl Chunk {
    pub(super) fn empty() -> Self {
        Self {
            cells: Box::new([0; CELLS]),
            filled: 0,
        }
    }

    pub(super) fn get(&self, index: usize) -> Option<Tile> {
        decode(self.cells[index])
    }

    /// Stores `tile` at `index`, returning what was there.
    pub(super) fn set(&mut self, index: usize, tile: Option<Tile>) -> Option<Tile> {
        let previous = decode(self.cells[index]);
        self.cells[index] = encode(tile);
        match (previous.is_some(), tile.is_some()) {
            (false, true) => self.filled += 1,
            (true, false) => self.filled -= 1,
            _ => {}
        }
        previous
    }

    pub(super) fn is_empty(&self) -> bool {
        self.filled == 0
    }

    pub(super) fn len(&self) -> usize {
        usize::from(self.filled)
    }

    /// Every filled cell, by index, in storage order.
    pub(super) fn tiles(&self) -> impl Iterator<Item = (usize, Tile)> + '_ {
        self.cells
            .iter()
            .enumerate()
            .filter_map(|(index, &raw)| Some((index, decode(raw)?)))
    }
}

fn encode(tile: Option<Tile>) -> u32 {
    let Some(tile) = tile else {
        return 0;
    };
    let mut raw = tile.frame + 1;
    if tile.flip_x {
        raw |= FLIP_X;
    }
    if tile.flip_y {
        raw |= FLIP_Y;
    }
    raw
}

fn decode(raw: u32) -> Option<Tile> {
    let frame = raw & FRAME_BITS;
    if frame == 0 {
        return None;
    }
    Some(Tile {
        frame: frame - 1,
        flip_x: raw & FLIP_X != 0,
        flip_y: raw & FLIP_Y != 0,
    })
}

/// One chunk as it appears in a file.
#[derive(Serialize, Deserialize)]
struct ChunkRecord {
    at: IVec2,
    cells: Vec<u32>,
}

/// The `chunks` field's wire form: a list of non-empty chunks in key order.
///
/// A list rather than a map: RON writes a map key as a full value, and the
/// order is the `BTreeMap`'s, so two saves of one map are byte-identical.
pub(super) mod layout {
    use super::*;

    pub(in crate::tilemap) fn serialize<S: Serializer>(
        chunks: &BTreeMap<ChunkKey, Chunk>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let records: Vec<ChunkRecord> = chunks
            .iter()
            .filter(|(_, chunk)| !chunk.is_empty())
            .map(|(key, chunk)| ChunkRecord {
                at: IVec2::new(key.0, key.1),
                cells: chunk.cells.to_vec(),
            })
            .collect();
        records.serialize(serializer)
    }

    /// Refuses a chunk of the wrong size and a chunk listed twice, rather
    /// than guessing which copy was meant; an empty chunk is dropped.
    pub(in crate::tilemap) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<ChunkKey, Chunk>, D::Error> {
        use serde::de::Error;

        let records = Vec::<ChunkRecord>::deserialize(deserializer)?;
        let mut chunks = BTreeMap::new();
        for record in records {
            let cells: Box<[u32; CELLS]> =
                record
                    .cells
                    .into_boxed_slice()
                    .try_into()
                    .map_err(|cells: Box<[u32]>| {
                        D::Error::custom(format!(
                            "a tilemap chunk holds {CELLS} cells, this one {}",
                            cells.len()
                        ))
                    })?;
            let filled = cells.iter().filter(|&&raw| decode(raw).is_some()).count();
            let chunk = Chunk {
                cells,
                filled: filled as u16,
            };
            if chunk.is_empty() {
                continue;
            }
            let key = (record.at.x, record.at.y);
            if chunks.insert(key, chunk).is_some() {
                return Err(D::Error::custom(format!(
                    "tilemap chunk ({}, {}) is listed twice",
                    key.0, key.1
                )));
            }
        }
        Ok(chunks)
    }
}
