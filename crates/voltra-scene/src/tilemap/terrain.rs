//! Auto-tiling: paint a terrain, and the edges pick themselves.
//!
//! Godot's "Match Sides" terrain mode and the 16-tile blob of every 2D tool:
//! a terrain names one frame for each combination of which of a cell's four
//! sides touch the same terrain. Painting or erasing a cell re-picks it and
//! its four neighbours, so a grass patch keeps its rim however it is drawn.

use voltra_render::glam::IVec2;

use super::{Tile, Tilemap};

/// The side bits of a cell's neighbourhood mask. Cell `y` grows **up**, as
/// the layer's space does.
pub const NORTH: usize = 1;
/// The east neighbour's bit, `x + 1`.
pub const EAST: usize = 2;
/// The south neighbour's bit, `y - 1`.
pub const SOUTH: usize = 4;
/// The west neighbour's bit, `x - 1`.
pub const WEST: usize = 8;

/// A set of frames that join up: `frames[mask]` is the frame for a cell
/// whose same-terrain neighbours are the sides set in `mask`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Terrain {
    /// What the terrain is called.
    pub name: String,
    /// The frame for each of the sixteen side masks, indexed by mask.
    pub frames: [u32; 16],
    /// The 47-tile "blob" set, which also looks at the diagonals: inner
    /// corners, the lake with an inlet. Empty keeps the sixteen-frame sides
    /// mode; otherwise exactly [`BLOB_FRAMES`] frames, in the order
    /// [`blob_index`] gives. Any other length is read as empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blob: Vec<u32>,
}

/// How many frames the corners-and-sides set has.
pub const BLOB_FRAMES: usize = 47;

/// The diagonal bits of the eight-neighbour mask, above the side bits.
pub const NORTH_EAST: usize = 16;
/// The south-east diagonal's bit.
pub const SOUTH_EAST: usize = 32;
/// The south-west diagonal's bit.
pub const SOUTH_WEST: usize = 64;
/// The north-west diagonal's bit.
pub const NORTH_WEST: usize = 128;

/// An eight-neighbour mask with every corner whose two sides are not both
/// set cleared: a diagonal only matters to a cell's shape when it closes a
/// corner between two sides that already join. Of the 256 masks, 47 remain.
pub fn reduce(mask: usize) -> usize {
    let side = |bit: usize| mask & bit != 0;
    let mut reduced = mask & (NORTH | EAST | SOUTH | WEST);
    for (corner, a, b) in [
        (NORTH_EAST, NORTH, EAST),
        (SOUTH_EAST, SOUTH, EAST),
        (SOUTH_WEST, SOUTH, WEST),
        (NORTH_WEST, NORTH, WEST),
    ] {
        if side(corner) && side(a) && side(b) {
            reduced |= corner;
        }
    }
    reduced
}

/// Every reduced mask, ascending: the order a blob terrain's frames are in.
/// Worked out once: every cell a stroke touches asks.
pub fn blob_masks() -> &'static [usize] {
    static MASKS: std::sync::OnceLock<Vec<usize>> = std::sync::OnceLock::new();
    MASKS.get_or_init(|| {
        let mut masks: Vec<usize> = (0..256).map(reduce).collect();
        masks.sort_unstable();
        masks.dedup();
        masks
    })
}

/// Which of a blob terrain's frames an eight-neighbour mask draws.
pub fn blob_index(mask: usize) -> usize {
    let reduced = reduce(mask);
    blob_masks()
        .binary_search(&reduced)
        .expect("a reduced mask is one of the reduced masks")
}

impl Terrain {
    /// A terrain drawing `frame` whatever its neighbours: the start a
    /// tileset's real edges are then filled into.
    pub fn uniform(name: impl Into<String>, frame: u32) -> Self {
        Self {
            name: name.into(),
            frames: [frame; 16],
            blob: Vec::new(),
        }
    }

    /// Whether this terrain uses the 47-tile set.
    pub fn is_blob(&self) -> bool {
        self.blob.len() == BLOB_FRAMES
    }

    /// Whether `frame` is one of this terrain's.
    pub fn owns(&self, frame: u32) -> bool {
        if self.is_blob() {
            self.blob.contains(&frame)
        } else {
            self.frames.contains(&frame)
        }
    }

    /// The frame for an eight-neighbour mask, in whichever mode this terrain
    /// is: the blob looks at every bit, the sides mode at the low four.
    pub fn frame_for(&self, mask: usize) -> u32 {
        if self.is_blob() {
            self.blob[blob_index(mask)]
        } else {
            self.frames[mask & 15]
        }
    }

    /// Any frame of the terrain, to mark a cell as belonging to it before
    /// its real frame is picked.
    fn any_frame(&self) -> u32 {
        if self.is_blob() {
            self.blob[0]
        } else {
            self.frames[0]
        }
    }
}

impl Tilemap {
    /// The terrain the tile at `cell` belongs to: the first whose frames
    /// include it. `None` for an empty cell or a tile no terrain names.
    pub fn terrain_of(&self, cell: IVec2) -> Option<usize> {
        let tile = self.get(cell)?;
        self.terrains.iter().position(|t| t.owns(tile.frame))
    }

    /// Paints `cell` with terrain `terrain` — or empties it for `None` — and
    /// re-picks the frame of it and of its four neighbours that belong to a
    /// terrain. Returns whether anything was written: a terrain index this
    /// layer does not have writes nothing.
    pub fn paint_terrain(&mut self, cell: IVec2, terrain: Option<usize>) -> bool {
        match terrain {
            Some(index) => {
                let Some(first) = self.terrains.get(index).map(Terrain::any_frame) else {
                    return false;
                };
                // Any frame of the terrain marks the cell as belonging to it;
                // the re-pick below chooses the right one.
                self.set(cell, Some(Tile::new(first)));
            }
            None => {
                self.set(cell, None);
            }
        }
        // The cell and all eight neighbours: a blob terrain's diagonals can
        // change with it, and a sides one simply ignores theirs.
        for y in -1..=1 {
            for x in -1..=1 {
                self.repick(cell + IVec2::new(x, y));
            }
        }
        true
    }

    /// Sets `cell`'s frame from its neighbours, if it belongs to a terrain.
    /// Flips are kept: a terrain says which frame, not which way round.
    fn repick(&mut self, cell: IVec2) {
        let Some(index) = self.terrain_of(cell) else {
            return;
        };
        let same = |at: IVec2| self.terrain_of(at) == Some(index);
        let around = [
            (IVec2::Y, NORTH),
            (IVec2::X, EAST),
            (-IVec2::Y, SOUTH),
            (-IVec2::X, WEST),
            (IVec2::new(1, 1), NORTH_EAST),
            (IVec2::new(1, -1), SOUTH_EAST),
            (IVec2::new(-1, -1), SOUTH_WEST),
            (IVec2::new(-1, 1), NORTH_WEST),
        ];
        let mask = around
            .into_iter()
            .filter(|(step, _)| same(cell + *step))
            .fold(0, |mask, (_, bit)| mask | bit);
        let frame = self.terrains[index].frame_for(mask);
        if let Some(mut tile) = self.get(cell) {
            if tile.frame != frame {
                tile.frame = frame;
                self.set(cell, Some(tile));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Frame `100 + mask`: the frame names its own neighbourhood.
    fn grass() -> Terrain {
        Terrain {
            name: "grass".to_owned(),
            frames: std::array::from_fn(|mask| 100 + mask as u32),
            blob: Vec::new(),
        }
    }

    fn layer() -> Tilemap {
        let mut map = Tilemap::default();
        map.terrains.push(grass());
        map
    }

    fn mask_at(map: &Tilemap, cell: IVec2) -> Option<u32> {
        map.get(cell).map(|t| t.frame - 100)
    }

    #[test]
    fn a_lone_cell_has_no_neighbours() {
        let mut map = layer();
        assert!(map.paint_terrain(IVec2::ZERO, Some(0)));
        assert_eq!(mask_at(&map, IVec2::ZERO), Some(0));
    }

    #[test]
    fn two_cells_side_by_side_face_each_other() {
        let mut map = layer();
        map.paint_terrain(IVec2::ZERO, Some(0));
        map.paint_terrain(IVec2::X, Some(0));
        assert_eq!(mask_at(&map, IVec2::ZERO), Some(EAST as u32));
        assert_eq!(mask_at(&map, IVec2::X), Some(WEST as u32));
    }

    #[test]
    fn north_is_up_the_layer() {
        let mut map = layer();
        map.paint_terrain(IVec2::ZERO, Some(0));
        map.paint_terrain(IVec2::Y, Some(0));
        assert_eq!(mask_at(&map, IVec2::ZERO), Some(NORTH as u32));
        assert_eq!(mask_at(&map, IVec2::Y), Some(SOUTH as u32));
    }

    #[test]
    fn a_cell_surrounded_is_the_full_mask() {
        let mut map = layer();
        for cell in [IVec2::ZERO, IVec2::X, -IVec2::X, IVec2::Y, -IVec2::Y] {
            map.paint_terrain(cell, Some(0));
        }
        assert_eq!(mask_at(&map, IVec2::ZERO), Some(15));
    }

    #[test]
    fn erasing_re_picks_the_neighbours() {
        let mut map = layer();
        map.paint_terrain(IVec2::ZERO, Some(0));
        map.paint_terrain(IVec2::X, Some(0));
        map.paint_terrain(IVec2::X, None);
        assert_eq!(map.get(IVec2::X), None);
        assert_eq!(mask_at(&map, IVec2::ZERO), Some(0), "alone again");
    }

    #[test]
    fn a_tile_of_no_terrain_is_neither_joined_nor_changed() {
        let mut map = layer();
        map.set(IVec2::X, Some(Tile::new(7)));
        map.paint_terrain(IVec2::ZERO, Some(0));
        assert_eq!(mask_at(&map, IVec2::ZERO), Some(0));
        assert_eq!(map.get(IVec2::X).map(|t| t.frame), Some(7));
    }

    #[test]
    fn two_terrains_do_not_join() {
        let mut map = layer();
        map.terrains.push(Terrain {
            name: "rock".to_owned(),
            frames: std::array::from_fn(|mask| 200 + mask as u32),
            blob: Vec::new(),
        });
        map.paint_terrain(IVec2::ZERO, Some(0));
        map.paint_terrain(IVec2::X, Some(1));
        assert_eq!(mask_at(&map, IVec2::ZERO), Some(0));
        assert_eq!(map.get(IVec2::X).map(|t| t.frame), Some(200));
    }

    #[test]
    fn an_unknown_terrain_writes_nothing() {
        let mut map = layer();
        assert!(!map.paint_terrain(IVec2::ZERO, Some(5)));
        assert!(map.is_empty());
    }

    #[test]
    fn the_corner_reduction_leaves_forty_seven_masks() {
        assert_eq!(blob_masks().len(), BLOB_FRAMES);
        assert_eq!(blob_index(0), 0, "alone is the first");
        assert_eq!(blob_index(255), BLOB_FRAMES - 1, "surrounded is the last");
    }

    #[test]
    fn a_corner_without_both_its_sides_does_not_count() {
        assert_eq!(reduce(NORTH_EAST), 0);
        assert_eq!(reduce(NORTH | NORTH_EAST), NORTH);
        assert_eq!(reduce(NORTH | EAST | NORTH_EAST), NORTH | EAST | NORTH_EAST);
    }

    fn blob_layer() -> Tilemap {
        let mut map = Tilemap::default();
        map.terrains.push(Terrain {
            name: "lake".to_owned(),
            frames: [0; 16],
            blob: (0..BLOB_FRAMES as u32).map(|i| 300 + i).collect(),
        });
        map
    }

    #[test]
    fn a_full_three_by_three_is_the_surrounded_frame_in_the_middle() {
        let mut map = blob_layer();
        for y in -1..=1 {
            for x in -1..=1 {
                map.paint_terrain(IVec2::new(x, y), Some(0));
            }
        }
        assert_eq!(map.get(IVec2::ZERO).map(|t| t.frame), Some(300 + 46));
    }

    #[test]
    fn a_missing_corner_is_an_inner_corner_not_a_full_cell() {
        let mut map = blob_layer();
        for y in -1..=1 {
            for x in -1..=1 {
                map.paint_terrain(IVec2::new(x, y), Some(0));
            }
        }
        map.paint_terrain(IVec2::new(1, 1), None);
        let expected = 300
            + blob_index(NORTH | EAST | SOUTH | WEST | SOUTH_EAST | SOUTH_WEST | NORTH_WEST) as u32;
        assert_eq!(map.get(IVec2::ZERO).map(|t| t.frame), Some(expected));
        assert_ne!(Some(expected), Some(300 + 46));
    }

    #[test]
    fn a_blob_of_the_wrong_length_falls_back_to_sides() {
        let mut terrain = grass();
        terrain.blob = vec![1, 2, 3];
        assert!(!terrain.is_blob());
        assert_eq!(terrain.frame_for(NORTH), 100 + NORTH as u32);
    }

    #[test]
    fn terrains_round_trip_and_an_old_layer_has_none() {
        let map = layer();
        let text = ron::to_string(&map).expect("writes");
        let back: Tilemap = ron::from_str(&text).expect("reads");
        assert_eq!(back.terrains, map.terrains);
        let plain = ron::to_string(&Tilemap::default()).expect("writes");
        assert!(!plain.contains("terrains"));
    }
}
