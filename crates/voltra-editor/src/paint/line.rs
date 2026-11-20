//! The cells a pointer crossed between two frames.
//!
//! A fast drag moves the pointer several cells in one frame. Painting only
//! the cell it ends on leaves a dotted line — the bug every first tile brush
//! ships with — so the stroke paints the whole segment, the way Tiled and
//! Godot do.

use voltra_render::glam::IVec2;

/// Every cell on the line from `from` to `to`, both ends included, each once,
/// in order. Bresenham's walk: one cell per step on the longer axis.
pub fn cells_between(from: IVec2, to: IVec2) -> Vec<IVec2> {
    let delta = to - from;
    let step = delta.signum();
    let run = delta.abs();
    let mut cells = Vec::with_capacity(run.max_element() as usize + 1);

    let mut at = from;
    // The error term of the classic integer form: positive means the minor
    // axis is due a step.
    let mut error = run.x - run.y;
    loop {
        cells.push(at);
        if at == to {
            return cells;
        }
        let twice = error * 2;
        if twice > -run.y {
            error -= run.y;
            at.x += step.x;
        }
        if twice < run.x {
            error += run.x;
            at.y += step.y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(x: i32, y: i32) -> IVec2 {
        IVec2::new(x, y)
    }

    #[test]
    fn a_line_of_no_length_is_its_one_cell() {
        assert_eq!(cells_between(c(3, -2), c(3, -2)), vec![c(3, -2)]);
    }

    #[test]
    fn a_horizontal_line_is_every_cell_along_it() {
        assert_eq!(
            cells_between(c(2, 0), c(-1, 0)),
            vec![c(2, 0), c(1, 0), c(0, 0), c(-1, 0)]
        );
    }

    #[test]
    fn a_diagonal_steps_both_axes_at_once() {
        assert_eq!(
            cells_between(c(0, 0), c(3, 3)),
            vec![c(0, 0), c(1, 1), c(2, 2), c(3, 3)]
        );
    }

    #[test]
    fn a_steep_line_has_no_gaps_on_its_long_axis() {
        let cells = cells_between(c(0, 0), c(2, -9));
        assert_eq!(cells.len(), 10);
        assert_eq!(cells.first(), Some(&c(0, 0)));
        assert_eq!(cells.last(), Some(&c(2, -9)));
        for pair in cells.windows(2) {
            let step = (pair[1] - pair[0]).abs();
            assert!(
                step.max_element() == 1,
                "jumped from {} to {}",
                pair[0],
                pair[1]
            );
        }
    }
}
