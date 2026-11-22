//! Whether a straight line between two points stays in free cells.

use voltra_render::glam::{IVec2, Vec2};

use super::NavGrid;

/// Below this, two crossings are one: the line passes through a corner.
const CORNER: f32 = 1e-6;

impl NavGrid {
    /// Whether every cell the segment `a`–`b` passes through is free.
    ///
    /// Every cell, corners included: a line through the point where four
    /// cells meet checks the two beside it, so a straightened path never
    /// squeezes diagonally between two blocked cells. Amanatides and Woo's
    /// traversal, which visits cells in order without sampling.
    pub fn line_of_sight(&self, a: Vec2, b: Vec2) -> bool {
        let (Some(mut cell), Some(end)) = (self.cell_of(a), self.cell_of(b)) else {
            return false;
        };
        let from = (a - self.origin) / self.cell;
        let delta = (b - self.origin) / self.cell - from;
        let step = IVec2::new(sign(delta.x), sign(delta.y));
        let mut t_max = Vec2::new(
            crossing(from.x, delta.x, cell.x),
            crossing(from.y, delta.y, cell.y),
        );
        let t_delta = Vec2::new(
            if delta.x != 0.0 {
                1.0 / delta.x.abs()
            } else {
                f32::INFINITY
            },
            if delta.y != 0.0 {
                1.0 / delta.y.abs()
            } else {
                f32::INFINITY
            },
        );
        let limit = (self.size.x + self.size.y) as usize * 2 + 4;
        for _ in 0..limit {
            if self.is_blocked(cell) {
                return false;
            }
            // The segment ends in this cell: at the end cell, or — for an
            // end on a cell's edge, where rounding can pick the neighbour —
            // wherever the next crossing would be past the end.
            if cell == end || t_max.x.min(t_max.y) > 1.0 {
                return true;
            }
            if (t_max.x - t_max.y).abs() < CORNER {
                // Through a corner: the cells either side of it as well.
                if self.is_blocked(cell + IVec2::new(step.x, 0))
                    || self.is_blocked(cell + IVec2::new(0, step.y))
                {
                    return false;
                }
                cell += step;
                t_max += t_delta;
            } else if t_max.x < t_max.y {
                cell.x += step.x;
                t_max.x += t_delta.x;
            } else {
                cell.y += step.y;
                t_max.y += t_delta.y;
            }
        }
        false
    }
}

fn sign(value: f32) -> i32 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

/// How far along a segment starting at `from` and moving `delta` it first
/// crosses out of cell `cell`, as a fraction of the segment.
fn crossing(from: f32, delta: f32, cell: i32) -> f32 {
    if delta > 0.0 {
        ((cell + 1) as f32 - from) / delta
    } else if delta < 0.0 {
        (cell as f32 - from) / delta
    } else {
        f32::INFINITY
    }
}
