//! A sheet cut into a grid of cells: as many as fit.

use voltra_assets::{Frame, Grid};
use voltra_render::glam::UVec2;

/// The most cells one slicing makes: a 1×1 cell on a 4096² sheet would be
/// sixteen million regions, which no window lists and no file should hold.
pub const MAX_CELLS: u32 = 1 << 16;

/// Every whole `cell` of a `sheet`, the first `offset` texels in from its
/// top-left and `spacing` texels apart, in reading order — Unity's Sprite
/// Editor's "Grid by cell size", Godot's atlas "Separation" and "Offset".
/// A cell cut off by the sheet's edge is left out rather than shortened:
/// a frame smaller than its neighbours would jump in an animation.
pub fn grid(
    sheet: UVec2,
    cell: UVec2,
    offset: UVec2,
    spacing: UVec2,
) -> Result<Vec<Frame>, String> {
    if cell.x == 0 || cell.y == 0 {
        return Err("a cell must be at least one texel each way".into());
    }
    let across = |sheet: u32, offset: u32, cell: u32, spacing: u32| {
        let room = sheet.saturating_sub(offset);
        if room < cell {
            0
        } else {
            (room - cell) / (cell + spacing) + 1
        }
    };
    let columns = across(sheet.x, offset.x, cell.x, spacing.x);
    let rows = across(sheet.y, offset.y, cell.y, spacing.y);
    if columns == 0 || rows == 0 {
        return Err(format!(
            "no {}×{} cell fits the {}×{} sheet at offset ({}, {})",
            cell.x, cell.y, sheet.x, sheet.y, offset.x, offset.y
        ));
    }
    if columns.saturating_mul(rows) > MAX_CELLS {
        return Err(format!(
            "{columns}×{rows} cells is more than the {MAX_CELLS} one slicing makes"
        ));
    }
    Ok(Grid::new(cell, columns, rows)
        .with_margin(offset)
        .with_padding(spacing)
        .frames())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheet_is_cut_into_every_whole_cell() {
        let frames = grid(
            UVec2::new(64, 16),
            UVec2::splat(16),
            UVec2::ZERO,
            UVec2::ZERO,
        )
        .expect("cells fit");
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[3], Frame::new(48, 0, 16, 16));
    }

    #[test]
    fn an_offset_and_spacing_move_the_cells_and_drop_a_cut_one() {
        // 2 + 16 + 3 + 16 + 3 + 16 = 56 of 60: three cells, and the fourth
        // would run off the edge.
        let frames = grid(
            UVec2::new(60, 20),
            UVec2::splat(16),
            UVec2::new(2, 1),
            UVec2::new(3, 0),
        )
        .expect("cells fit");
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[1], Frame::new(21, 1, 16, 16));
    }

    #[test]
    fn a_cell_bigger_than_the_sheet_or_of_no_size_is_refused() {
        assert!(grid(UVec2::splat(8), UVec2::splat(16), UVec2::ZERO, UVec2::ZERO).is_err());
        assert!(grid(UVec2::splat(8), UVec2::new(0, 4), UVec2::ZERO, UVec2::ZERO).is_err());
        assert!(grid(
            UVec2::splat(8),
            UVec2::splat(4),
            UVec2::splat(8),
            UVec2::ZERO
        )
        .is_err());
    }

    #[test]
    fn a_slicing_into_millions_is_refused() {
        assert!(grid(UVec2::splat(4096), UVec2::ONE, UVec2::ZERO, UVec2::ZERO).is_err());
    }
}
