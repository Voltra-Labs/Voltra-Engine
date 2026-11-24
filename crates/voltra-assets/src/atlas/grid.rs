//! A uniform grid of cells.

use serde::{Deserialize, Serialize};
use voltra_render::glam::UVec2;

use super::Frame;

/// A uniform grid of cells, which is what most sheets are.
///
/// `margin` is the border before the first cell; `padding` is the gap between
/// cells. Both exist because the sheets that are *not* a plain grid are
/// usually a grid with bleed guards, and re-cutting one by hand into explicit
/// rectangles is exactly the work this avoids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grid {
    /// Size of one cell, in texels.
    pub cell: UVec2,
    /// Cells across.
    pub columns: u32,
    /// Cells down.
    pub rows: u32,
    /// Border before the first cell, in texels.
    #[serde(default)]
    pub margin: UVec2,
    /// Gap between neighbouring cells, in texels.
    #[serde(default)]
    pub padding: UVec2,
}

impl Grid {
    /// `columns` × `rows` cells of `cell` texels, with no margin or padding.
    pub const fn new(cell: UVec2, columns: u32, rows: u32) -> Self {
        Self {
            cell,
            columns,
            rows,
            margin: UVec2::ZERO,
            padding: UVec2::ZERO,
        }
    }

    /// The grid with `margin` as its border before the first cell.
    pub const fn with_margin(mut self, margin: UVec2) -> Self {
        self.margin = margin;
        self
    }

    /// The grid with `padding` as its gap between cells.
    pub const fn with_padding(mut self, padding: UVec2) -> Self {
        self.padding = padding;
        self
    }

    /// The cells, left to right and then top to bottom — reading order, which
    /// is the order a sheet is drawn in and therefore the order frame indices
    /// have to follow.
    ///
    /// A grid with no cells, no columns or no rows cuts nothing rather than
    /// producing empty frames nobody can draw.
    pub fn frames(&self) -> Vec<Frame> {
        if self.cell.x == 0 || self.cell.y == 0 {
            return Vec::new();
        }
        let mut frames = Vec::with_capacity((self.columns * self.rows) as usize);
        for row in 0..self.rows {
            for column in 0..self.columns {
                frames.push(Frame::new(
                    self.margin.x + column * (self.cell.x + self.padding.x),
                    self.margin.y + row * (self.cell.y + self.padding.y),
                    self.cell.x,
                    self.cell.y,
                ));
            }
        }
        frames
    }

    /// The sheet a full grid needs, in texels: the margin on both sides, every
    /// cell, and the gaps between them.
    pub fn sheet(&self) -> UVec2 {
        if self.columns == 0 || self.rows == 0 {
            return UVec2::ZERO;
        }
        let cells = UVec2::new(self.columns, self.rows);
        self.margin * 2 + cells * self.cell + (cells - UVec2::ONE) * self.padding
    }
}
