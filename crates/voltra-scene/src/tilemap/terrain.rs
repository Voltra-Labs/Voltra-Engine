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
pub const EAST: usize = 2;
pub const SOUTH: usize = 4;
pub const WEST: usize = 8;

/// A set of frames that join up: `frames[mask]` is the frame for a cell
/// whose same-terrain neighbours are the sides set in `mask`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Terrain {
    pub name: String,
    pub frames: [u32; 16],
}

impl Terrain {
    /// A terrain drawing `frame` whatever its neighbours: the start a
    /// tileset's real edges are then filled into.
    pub fn uniform(name: impl Into<String>, frame: u32) -> Self {
        Self {
            name: name.into(),
            frames: [frame; 16],
        }
    }

    /// Whether `frame` is one of this terrain's.
    pub fn owns(&self, frame: u32) -> bool {
        self.frames.contains(&frame)
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
                let Some(first) = self.terrains.get(index).map(|t| t.frames[0]) else {
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
        let around = [
            cell,
            cell + IVec2::Y,
            cell + IVec2::X,
            cell - IVec2::Y,
            cell - IVec2::X,
        ];
        for at in around {
            self.repick(at);
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
        let sides = [
            (IVec2::Y, NORTH),
            (IVec2::X, EAST),
            (-IVec2::Y, SOUTH),
            (-IVec2::X, WEST),
        ];
        let mask = sides
            .into_iter()
            .filter(|(step, _)| same(cell + *step))
            .fold(0, |mask, (_, bit)| mask | bit);
        let frame = self.terrains[index].frames[mask];
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
    fn terrains_round_trip_and_an_old_layer_has_none() {
        let map = layer();
        let text = ron::to_string(&map).expect("writes");
        let back: Tilemap = ron::from_str(&text).expect("reads");
        assert_eq!(back.terrains, map.terrains);
        let plain = ron::to_string(&Tilemap::default()).expect("writes");
        assert!(!plain.contains("terrains"));
    }
}
