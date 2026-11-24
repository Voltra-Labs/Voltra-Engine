//! A grid of free and blocked cells, baked from the scene's colliders.

mod astar;
mod bake;
mod sight;

use voltra_render::glam::{IVec2, Vec2};

pub use bake::NavSettings;

/// Cells over a rectangle of the world, each free or blocked.
#[derive(Debug, Clone, PartialEq)]
pub struct NavGrid {
    /// The lower-left corner of cell `(0, 0)`, in world units.
    origin: Vec2,
    /// The side of a cell, in world units.
    cell: f32,
    /// Cells across and up.
    size: IVec2,
    /// Row by row from the bottom: whether each cell is blocked.
    blocked: Vec<bool>,
    /// Whether a path may step diagonally.
    diagonal: bool,
}

impl NavGrid {
    /// A grid of `size` free cells of side `cell` from `origin`.
    ///
    /// A cell of no size, or one that is not finite, is a cell of one unit;
    /// a negative size is an empty grid.
    pub fn new(origin: Vec2, cell: f32, size: IVec2) -> Self {
        let cell = if cell.is_finite() && cell > 0.0 {
            cell
        } else {
            1.0
        };
        let size = size.max(IVec2::ZERO);
        Self {
            origin,
            cell,
            size,
            blocked: vec![false; (size.x as usize) * (size.y as usize)],
            diagonal: true,
        }
    }

    /// Whether paths may step diagonally — never across a blocked corner.
    pub fn with_diagonal(mut self, diagonal: bool) -> Self {
        self.diagonal = diagonal;
        self
    }

    /// The world position of the grid's bottom-left corner.
    pub fn origin(&self) -> Vec2 {
        self.origin
    }

    /// The side of a cell, in world units.
    pub fn cell_size(&self) -> f32 {
        self.cell
    }

    /// Cells across and up.
    pub fn size(&self) -> IVec2 {
        self.size
    }

    /// The cell holding the world point `at`, if the grid covers it.
    pub fn cell_of(&self, at: Vec2) -> Option<IVec2> {
        let cell = ((at - self.origin) / self.cell).floor();
        if !cell.is_finite() {
            return None;
        }
        let cell = cell.as_ivec2();
        self.contains(cell).then_some(cell)
    }

    /// The centre of `cell`, in the world.
    pub fn centre(&self, cell: IVec2) -> Vec2 {
        self.origin + (cell.as_vec2() + Vec2::splat(0.5)) * self.cell
    }

    /// Whether `cell` is on the grid, blocked or not.
    pub fn contains(&self, cell: IVec2) -> bool {
        cell.x >= 0 && cell.y >= 0 && cell.x < self.size.x && cell.y < self.size.y
    }

    /// Whether `cell` is blocked; a cell off the grid is.
    pub fn is_blocked(&self, cell: IVec2) -> bool {
        self.index(cell).is_none_or(|i| self.blocked[i])
    }

    /// Blocks or frees `cell` — a door shut, a crate pushed. A cell off the
    /// grid is ignored.
    pub fn set_blocked(&mut self, cell: IVec2, blocked: bool) {
        if let Some(i) = self.index(cell) {
            self.blocked[i] = blocked;
        }
    }

    /// How many cells are blocked.
    pub fn blocked_count(&self) -> usize {
        self.blocked.iter().filter(|&&b| b).count()
    }

    fn index(&self, cell: IVec2) -> Option<usize> {
        self.contains(cell)
            .then(|| (cell.y as usize) * (self.size.x as usize) + cell.x as usize)
    }

    /// The free cell nearest `cell`, searching outwards ring by ring up to
    /// `reach` cells — where an agent standing a little inside a wall, or a
    /// goal placed on one, is taken to be.
    pub fn nearest_free(&self, cell: IVec2, reach: i32) -> Option<IVec2> {
        if !self.is_blocked(cell) {
            return Some(cell);
        }
        for ring in 1..=reach.max(0) {
            let mut best: Option<(i32, IVec2)> = None;
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    if dx.abs() != ring && dy.abs() != ring {
                        continue;
                    }
                    let candidate = cell + IVec2::new(dx, dy);
                    if self.is_blocked(candidate) {
                        continue;
                    }
                    let distance = dx * dx + dy * dy;
                    if best.is_none_or(|(d, _)| distance < d) {
                        best = Some((distance, candidate));
                    }
                }
            }
            if let Some((_, found)) = best {
                return Some(found);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
