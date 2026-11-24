//! A tileset laid out as its sheet is, so a rectangle of it is a stamp.
//!
//! Frames are grouped into columns by their left edge and into rows by
//! their top edge: a grid atlas comes out as its grid, and a packed one as
//! the nearest grid its frames make. Frames with no texels — a re-pack's
//! kept slots — are left out.

use std::collections::BTreeSet;

use voltra_assets::Atlas;
use voltra_render::glam::UVec2;
use voltra_scene::Tile;

use super::stamp::Stamp;

/// The palette's cells, each the frame drawn there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteGrid {
    columns: u32,
    rows: u32,
    /// Row by row from the top, as the sheet reads.
    cells: Vec<Option<u32>>,
}

impl PaletteGrid {
    /// `atlas`'s frames in the places their sheet has them.
    pub fn of(atlas: &Atlas) -> Self {
        let frames: Vec<(u32, voltra_assets::Frame)> = atlas
            .frames()
            .iter()
            .enumerate()
            .filter(|(_, frame)| !frame.is_empty())
            .map(|(i, frame)| (i as u32, *frame))
            .collect();
        let xs: Vec<u32> = frames
            .iter()
            .map(|(_, f)| f.x)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let ys: Vec<u32> = frames
            .iter()
            .map(|(_, f)| f.y)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let columns = xs.len() as u32;
        let rows = ys.len() as u32;
        let mut cells = vec![None; (columns * rows) as usize];
        for (index, frame) in frames {
            let column = xs.binary_search(&frame.x).expect("every x was collected");
            let row = ys.binary_search(&frame.y).expect("every y was collected");
            let cell = &mut cells[row * columns as usize + column];
            // Two frames at one corner: the first keeps the cell.
            if cell.is_none() {
                *cell = Some(index);
            }
        }
        Self {
            columns,
            rows,
            cells,
        }
    }

    /// Columns and rows.
    pub fn size(&self) -> UVec2 {
        UVec2::new(self.columns, self.rows)
    }

    /// The frame at `cell`, counted from the top-left.
    pub fn frame_at(&self, cell: UVec2) -> Option<u32> {
        if cell.x >= self.columns || cell.y >= self.rows {
            return None;
        }
        self.cells[(cell.y * self.columns + cell.x) as usize]
    }

    /// Where `frame` is, if it is on the palette.
    pub fn position_of(&self, frame: u32) -> Option<UVec2> {
        let i = self.cells.iter().position(|cell| *cell == Some(frame))? as u32;
        Some(UVec2::new(i % self.columns, i / self.columns))
    }

    /// The rectangle of cells from `a` to `b`, either way round, as a stamp
    /// laid out as it looks — its top row on top. `None` when it is all
    /// holes or off the palette.
    pub fn stamp(&self, a: UVec2, b: UVec2) -> Option<Stamp> {
        let min = a.min(b);
        let max = a.max(b).min(self.size().saturating_sub(UVec2::ONE));
        if self.columns == 0 || self.rows == 0 || min.cmpgt(max).any() {
            return None;
        }
        let size = max - min + UVec2::ONE;
        // The stamp's rows run up; the palette's run down.
        let tiles = (0..size.y)
            .flat_map(|y| (0..size.x).map(move |x| UVec2::new(min.x + x, max.y - y)))
            .map(|cell| self.frame_at(cell).map(Tile::new))
            .collect();
        Stamp::new(size, tiles)
    }
}

#[cfg(test)]
mod tests {
    use voltra_assets::{Frame, Grid};

    use super::*;

    #[test]
    fn a_grid_atlas_lays_out_as_its_grid() {
        let atlas = Atlas::from_grid(Grid::new(UVec2::splat(16), 4, 2));
        let grid = PaletteGrid::of(&atlas);

        assert_eq!(grid.size(), UVec2::new(4, 2));
        assert_eq!(grid.frame_at(UVec2::new(1, 1)), Some(5));
        assert_eq!(grid.position_of(6), Some(UVec2::new(2, 1)));
    }

    #[test]
    fn a_rectangle_of_the_palette_is_a_stamp_the_right_way_up() {
        let atlas = Atlas::from_grid(Grid::new(UVec2::splat(16), 4, 2));
        let stamp = PaletteGrid::of(&atlas)
            .stamp(UVec2::new(3, 1), UVec2::new(1, 0))
            .expect("tiles");

        assert_eq!(stamp.size(), UVec2::new(3, 2));
        // The palette's top row (frames 1 2 3) is the stamp's top row.
        assert_eq!(stamp.get(UVec2::new(0, 1)).map(|t| t.frame), Some(1));
        assert_eq!(stamp.get(UVec2::new(2, 0)).map(|t| t.frame), Some(7));
    }

    #[test]
    fn an_empty_frame_is_not_on_the_palette() {
        let atlas = Atlas::new(
            UVec2::splat(32),
            vec![
                Frame::new(0, 0, 16, 16),
                Frame::default(),
                Frame::new(16, 0, 16, 16),
            ],
        );
        let grid = PaletteGrid::of(&atlas);
        assert_eq!(grid.size(), UVec2::new(2, 1));
        assert_eq!(grid.position_of(1), None);
        assert_eq!(grid.frame_at(UVec2::new(1, 0)), Some(2));
    }
}
