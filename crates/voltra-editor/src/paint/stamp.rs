//! What the brush paints: one tile, or a rectangle of them picked from the
//! palette or off the map, turned and mirrored as a whole.
//!
//! Tiled's stamp and Unity's brush pick: a block of tiles painted at once,
//! its holes left alone, so a doorway or a tree three tiles tall goes down
//! in one click. Cells run `y` up, as the map's do: `(0, 0)` is the stamp's
//! bottom-left, and that is the cell under the pointer when it is painted.

use voltra_render::glam::{IVec2, UVec2};
use voltra_scene::{Tile, Tilemap};

/// A `size` block of tiles, row by row from the bottom; `None` is a hole,
/// which painting leaves as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    size: UVec2,
    tiles: Vec<Option<Tile>>,
}

impl Default for Stamp {
    /// Frame 0, the tile a fresh brush has always painted.
    fn default() -> Self {
        Self::single(Tile::new(0))
    }
}

impl Stamp {
    /// The largest side a stamp may have: what a palette drag or a pick off
    /// the map can make, kept from becoming a million-cell paste by a slip.
    pub const MAX_SIDE: u32 = 256;

    /// One tile.
    pub fn single(tile: Tile) -> Self {
        Self {
            size: UVec2::ONE,
            tiles: vec![Some(tile)],
        }
    }

    /// A `size` block from `tiles`, row by row from the bottom. `None` if
    /// the counts disagree, a side is zero or past [`Self::MAX_SIDE`], or
    /// every cell is a hole — a stamp that paints nothing.
    pub fn new(size: UVec2, tiles: Vec<Option<Tile>>) -> Option<Self> {
        let fits = size.x > 0
            && size.y > 0
            && size.max_element() <= Self::MAX_SIDE
            && tiles.len() == (size.x * size.y) as usize
            && tiles.iter().any(Option::is_some);
        fits.then_some(Self { size, tiles })
    }

    /// The cells `min..=max` of `map`, as they are: what the eyedropper
    /// picks. `None` if they are all empty or too many.
    pub fn from_map(map: &Tilemap, a: IVec2, b: IVec2) -> Option<Self> {
        let min = a.min(b);
        let max = a.max(b);
        let size = (max.as_i64vec2() - min.as_i64vec2() + 1).as_uvec2();
        if size.max_element() > Self::MAX_SIDE {
            return None;
        }
        let tiles = (0..size.y as i32)
            .flat_map(|y| (0..size.x as i32).map(move |x| IVec2::new(x, y)))
            .map(|offset| map.get(min + offset))
            .collect();
        Self::new(size, tiles)
    }

    /// Cells across and up.
    pub fn size(&self) -> UVec2 {
        self.size
    }

    /// The tile at `cell` of the stamp, if it is not a hole.
    pub fn get(&self, cell: UVec2) -> Option<Tile> {
        if cell.x >= self.size.x || cell.y >= self.size.y {
            return None;
        }
        self.tiles[(cell.y * self.size.x + cell.x) as usize]
    }

    /// The tile, if the stamp is one tile: what the inspector's palette
    /// highlights and a new terrain starts from.
    pub fn single_tile(&self) -> Option<Tile> {
        (self.size == UVec2::ONE).then(|| self.tiles[0]).flatten()
    }

    /// The first tile there is, for whatever wants one tile of the stamp.
    pub fn first(&self) -> Tile {
        self.tiles
            .iter()
            .flatten()
            .next()
            .copied()
            .unwrap_or_default()
    }

    /// Every cell and its tile, holes left out.
    pub fn tiles(&self) -> impl Iterator<Item = (UVec2, Tile)> + '_ {
        let width = self.size.x;
        self.tiles.iter().enumerate().filter_map(move |(i, tile)| {
            let i = i as u32;
            Some((UVec2::new(i % width, i / width), (*tile)?))
        })
    }

    /// The stamp turned a quarter clockwise, as seen: the block and every
    /// tile in it.
    pub fn rotated_cw(&self) -> Self {
        // A cell (x, y), y up, goes to (y, w - 1 - x) in an h × w block.
        let (w, h) = (self.size.x, self.size.y);
        self.remapped(UVec2::new(h, w), |x, y| (y, w - 1 - x), Tile::rotated_cw)
    }

    /// The stamp turned a quarter anticlockwise, as seen.
    pub fn rotated_ccw(&self) -> Self {
        let (w, h) = (self.size.x, self.size.y);
        self.remapped(UVec2::new(h, w), |x, y| (h - 1 - y, x), Tile::rotated_ccw)
    }

    /// The stamp mirrored left to right, as seen.
    pub fn mirrored_x(&self) -> Self {
        let w = self.size.x;
        self.remapped(self.size, |x, y| (w - 1 - x, y), Tile::mirrored_x)
    }

    /// The stamp mirrored top to bottom, as seen.
    pub fn mirrored_y(&self) -> Self {
        let h = self.size.y;
        self.remapped(self.size, |x, y| (x, h - 1 - y), Tile::mirrored_y)
    }

    fn remapped(
        &self,
        size: UVec2,
        to: impl Fn(u32, u32) -> (u32, u32),
        turn: impl Fn(Tile) -> Tile,
    ) -> Self {
        let mut tiles = vec![None; self.tiles.len()];
        for (cell, tile) in self.tiles() {
            let (x, y) = to(cell.x, cell.y);
            tiles[(y * size.x + x) as usize] = Some(turn(tile));
        }
        Self { size, tiles }
    }

    /// Paints the stamp with its bottom-left on `at`. Holes are left alone.
    pub fn paint(&self, map: &mut Tilemap, at: IVec2) {
        for (cell, tile) in self.tiles() {
            map.set(at + cell.as_ivec2(), Some(tile));
        }
    }

    /// The stamp's tile for `cell` when it is laid over a whole area as a
    /// repeating pattern from `origin`: what a rectangle or a flood fill
    /// paints. A hole paints nothing there.
    pub fn pattern(&self, origin: IVec2, cell: IVec2) -> Option<Tile> {
        let size = self.size.as_ivec2();
        let at = (cell - origin).rem_euclid(size);
        self.get(at.as_uvec2())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_render::glam::Vec2;

    /// 3 across, 2 up: frames 0 1 2 on the bottom row, 3 4 5 above.
    fn three_by_two() -> Stamp {
        Stamp::new(
            UVec2::new(3, 2),
            (0..6).map(|f| Some(Tile::new(f))).collect(),
        )
        .expect("a block")
    }

    #[test]
    fn a_stamp_paints_its_block_from_its_bottom_left() {
        let mut map = Tilemap::new(Vec2::ONE);
        three_by_two().paint(&mut map, IVec2::new(10, -1));

        assert_eq!(map.len(), 6);
        assert_eq!(map.get(IVec2::new(10, -1)), Some(Tile::new(0)));
        assert_eq!(map.get(IVec2::new(12, 0)), Some(Tile::new(5)));
    }

    #[test]
    fn a_hole_leaves_the_cell_alone() {
        let mut map = Tilemap::new(Vec2::ONE);
        map.set(IVec2::new(1, 0), Some(Tile::new(9)));
        let stamp = Stamp::new(UVec2::new(2, 1), vec![Some(Tile::new(1)), None]).expect("a stamp");

        stamp.paint(&mut map, IVec2::ZERO);

        assert_eq!(map.get(IVec2::new(1, 0)), Some(Tile::new(9)));
    }

    #[test]
    fn turning_a_block_turns_its_layout_and_its_tiles() {
        let turned = three_by_two().rotated_cw();

        assert_eq!(turned.size(), UVec2::new(2, 3));
        // The old top-left (frame 3, at (0, 1)) is now the top-right.
        assert_eq!(turned.get(UVec2::new(1, 2)).map(|t| t.frame), Some(3));
        // The old bottom-left (frame 0) is now the top-left.
        assert_eq!(turned.get(UVec2::new(0, 2)).map(|t| t.frame), Some(0));
        assert!(turned.get(UVec2::new(0, 2)).is_some_and(|t| t.transpose));
        assert_eq!(turned.rotated_ccw(), three_by_two());
        assert_eq!(
            three_by_two()
                .rotated_cw()
                .rotated_cw()
                .rotated_cw()
                .rotated_cw(),
            three_by_two()
        );
    }

    #[test]
    fn mirroring_a_block_reverses_its_rows_or_columns() {
        let mirrored = three_by_two().mirrored_x();
        assert_eq!(mirrored.get(UVec2::ZERO), Some(Tile::new(2).mirrored_x()));
        let mirrored = three_by_two().mirrored_y();
        assert_eq!(mirrored.get(UVec2::ZERO).map(|t| t.frame), Some(3));
        assert_eq!(mirrored.mirrored_y(), three_by_two());
    }

    #[test]
    fn a_stamp_picked_off_the_map_is_what_was_there() {
        let mut map = Tilemap::new(Vec2::ONE);
        three_by_two().paint(&mut map, IVec2::new(4, 4));

        let picked = Stamp::from_map(&map, IVec2::new(6, 5), IVec2::new(4, 4)).expect("tiles");

        assert_eq!(picked, three_by_two());
        assert!(
            Stamp::from_map(&map, IVec2::ZERO, IVec2::ONE).is_none(),
            "all empty"
        );
    }

    #[test]
    fn a_pattern_repeats_from_its_origin() {
        let stamp = three_by_two();
        assert_eq!(
            stamp
                .pattern(IVec2::ZERO, IVec2::new(3, 0))
                .map(|t| t.frame),
            Some(0)
        );
        assert_eq!(
            stamp
                .pattern(IVec2::ZERO, IVec2::new(-1, -1))
                .map(|t| t.frame),
            Some(5)
        );
    }

    #[test]
    fn a_stamp_of_nothing_or_of_the_wrong_count_is_not_one() {
        assert!(Stamp::new(UVec2::new(2, 1), vec![None, None]).is_none());
        assert!(Stamp::new(UVec2::new(2, 2), vec![Some(Tile::new(0))]).is_none());
        assert!(Stamp::new(UVec2::ZERO, Vec::new()).is_none());
    }
}
