//! A* over the free cells, and the path it finds made straight.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use voltra_render::glam::{IVec2, Vec2};

use super::NavGrid;
use crate::path::Path;

/// How far from a blocked start or goal a free cell is looked for, in cells.
const NEAREST: i32 = 4;

const SQRT_2: f32 = std::f32::consts::SQRT_2;

/// A cell waiting to be expanded, cheapest estimate first.
#[derive(Clone, Copy)]
struct Open {
    estimate: f32,
    index: usize,
}

impl PartialEq for Open {
    fn eq(&self, other: &Self) -> bool {
        self.estimate.total_cmp(&other.estimate) == Ordering::Equal
    }
}

impl Eq for Open {}

impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Open {
    /// Reversed: `BinaryHeap` is a max-heap, and the cheapest comes first.
    fn cmp(&self, other: &Self) -> Ordering {
        other.estimate.total_cmp(&self.estimate)
    }
}

impl NavGrid {
    /// The way from `from` to `to`, straightened where the agent can see
    /// ahead. `None` when either point is off the grid, or no way joins
    /// them.
    ///
    /// A start or goal a little inside a blocked cell — an agent pressed
    /// against a wall, a target on one — is taken from the nearest free
    /// cell; the path then ends at that cell's centre rather than at `to`.
    pub fn find_path(&self, from: Vec2, to: Vec2) -> Option<Path> {
        self.search(from, to, false)
    }

    /// [`find_path`](Self::find_path), but when `to` cannot be reached, the
    /// way to the reachable cell nearest it — what a chaser does when its
    /// target is behind a wall. `None` only when `from` is off the grid or
    /// boxed in.
    pub fn find_path_closest(&self, from: Vec2, to: Vec2) -> Option<Path> {
        self.search(from, to, true)
    }

    fn search(&self, from: Vec2, to: Vec2, closest: bool) -> Option<Path> {
        let start = self.nearest_free(self.cell_of(from)?, NEAREST)?;
        let wanted = match self.cell_of(to) {
            Some(cell) => cell,
            None if closest => self.clamp_cell(to),
            None => return None,
        };
        let goal = self.nearest_free(wanted, NEAREST);
        let (cells, reached) = match goal {
            Some(goal) => match self.astar(start, goal) {
                Some(cells) => (cells, true),
                None if closest => (self.astar_closest(start, wanted), false),
                None => return None,
            },
            None if closest => (self.astar_closest(start, wanted), false),
            None => return None,
        };
        let exact = reached && goal == Some(wanted) && self.cell_of(to) == Some(wanted);
        let mut points = Vec::with_capacity(cells.len() + 1);
        points.push(from);
        points.extend(cells.iter().skip(1).map(|&c| self.centre(c)));
        if exact {
            match points.len() {
                1 => points.push(to),
                _ => {
                    let last = points.len() - 1;
                    points[last] = to;
                }
            }
        }
        Some(Path::new(self.straighten(points)))
    }

    /// The cell on the grid nearest the world point `at`.
    fn clamp_cell(&self, at: Vec2) -> IVec2 {
        let cell = ((at - self.origin) / self.cell).floor();
        let cell = if cell.is_finite() {
            cell.as_ivec2()
        } else {
            IVec2::ZERO
        };
        cell.clamp(IVec2::ZERO, (self.size - IVec2::ONE).max(IVec2::ZERO))
    }

    fn astar(&self, start: IVec2, goal: IVec2) -> Option<Vec<IVec2>> {
        let (came_from, found) = self.expand(start, goal);
        found.then(|| self.walk_back(&came_from, goal))
    }

    /// The way to the cell nearest `goal` that `start` reaches.
    fn astar_closest(&self, start: IVec2, goal: IVec2) -> Vec<IVec2> {
        let (came_from, _) = self.expand(start, goal);
        let nearest = (0..came_from.len())
            .filter(|&i| came_from[i] != NONE || i == self.flat(start))
            .map(|i| self.unflat(i))
            .min_by(|a, b| {
                let da = (*a - goal).as_vec2().length_squared();
                let db = (*b - goal).as_vec2().length_squared();
                da.total_cmp(&db)
            })
            .unwrap_or(start);
        self.walk_back(&came_from, nearest)
    }

    /// Runs A* from `start` towards `goal`; answers where each reached cell
    /// came from, and whether `goal` was reached.
    fn expand(&self, start: IVec2, goal: IVec2) -> (Vec<usize>, bool) {
        let count = self.blocked.len();
        let mut came_from = vec![NONE; count];
        let mut cost = vec![f32::INFINITY; count];
        let mut closed = vec![false; count];
        let mut open = BinaryHeap::new();
        let first = self.flat(start);
        cost[first] = 0.0;
        open.push(Open {
            estimate: self.heuristic(start, goal),
            index: first,
        });
        while let Some(Open { index, .. }) = open.pop() {
            if closed[index] {
                continue;
            }
            closed[index] = true;
            let cell = self.unflat(index);
            if cell == goal {
                return (came_from, true);
            }
            for (step, length) in self.steps() {
                let next = cell + step;
                if self.is_blocked(next) {
                    continue;
                }
                // A diagonal never cuts a blocked corner.
                if step.x != 0
                    && step.y != 0
                    && (self.is_blocked(cell + IVec2::new(step.x, 0))
                        || self.is_blocked(cell + IVec2::new(0, step.y)))
                {
                    continue;
                }
                let n = self.flat(next);
                let through = cost[index] + length;
                if through < cost[n] {
                    cost[n] = through;
                    came_from[n] = index;
                    open.push(Open {
                        estimate: through + self.heuristic(next, goal),
                        index: n,
                    });
                }
            }
        }
        (came_from, false)
    }

    fn steps(&self) -> impl Iterator<Item = (IVec2, f32)> {
        let straight = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y].map(|s| (s, 1.0));
        let diagonal = [
            IVec2::new(1, 1),
            IVec2::new(1, -1),
            IVec2::new(-1, 1),
            IVec2::new(-1, -1),
        ]
        .map(|s| (s, SQRT_2));
        let diagonals = if self.diagonal { 4 } else { 0 };
        straight
            .into_iter()
            .chain(diagonal.into_iter().take(diagonals))
    }

    /// Octile distance with diagonals, Manhattan without: never more than
    /// the real cost, so the path found is a shortest one.
    fn heuristic(&self, a: IVec2, b: IVec2) -> f32 {
        let d = (a - b).abs();
        let (lo, hi) = (d.x.min(d.y) as f32, d.x.max(d.y) as f32);
        if self.diagonal {
            hi - lo + lo * SQRT_2
        } else {
            hi + lo
        }
    }

    fn walk_back(&self, came_from: &[usize], end: IVec2) -> Vec<IVec2> {
        let mut cells = vec![end];
        let mut at = self.flat(end);
        while came_from[at] != NONE {
            at = came_from[at];
            cells.push(self.unflat(at));
        }
        cells.reverse();
        cells
    }

    fn flat(&self, cell: IVec2) -> usize {
        (cell.y as usize) * (self.size.x as usize) + cell.x as usize
    }

    fn unflat(&self, index: usize) -> IVec2 {
        let width = self.size.x.max(1) as usize;
        IVec2::new((index % width) as i32, (index / width) as i32)
    }

    /// Drops every waypoint the one before it can see past: the agent walks
    /// straight across open ground instead of from cell centre to cell
    /// centre.
    fn straighten(&self, points: Vec<Vec2>) -> Vec<Vec2> {
        if points.len() <= 2 {
            return points;
        }
        let mut kept = vec![points[0]];
        let mut at = 0;
        while at < points.len() - 1 {
            let mut farthest = at + 1;
            for next in at + 2..points.len() {
                if self.line_of_sight(points[at], points[next]) {
                    farthest = next;
                }
            }
            kept.push(points[farthest]);
            at = farthest;
        }
        kept
    }
}

/// No cell: where the start came from.
const NONE: usize = usize::MAX;
