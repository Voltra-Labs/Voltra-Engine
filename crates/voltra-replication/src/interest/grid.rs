//! A uniform grid over the entities a server replicates, answering which of
//! them are near a client's points.
//!
//! Rebuilt every snapshot rather than kept up to date: inserting a thousand
//! entities is a thousand hash pushes, cheaper than tracking which moved
//! out of which cell, and a rebuilt grid cannot drift from the world.
//! Unreal's replication graph keeps a grid node for the same question;
//! Godot and Unity Netcode leave it to a per-pair visibility callback.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

use glam::Vec2;

use super::Interest;
use crate::NetId;

/// A cell's coordinates hashed with one multiply per word: SipHash's
/// defence against chosen keys buys nothing for cells the server computes
/// itself, and costs a third of a rebuild.
#[derive(Debug, Default, Clone, Copy)]
struct CellHasher(u64);

impl Hasher for CellHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.mix(u64::from(byte));
        }
    }

    fn write_i32(&mut self, value: i32) {
        self.mix(u64::from(value as u32));
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

impl CellHasher {
    fn mix(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(26) ^ word).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
}

type Cells = HashMap<(i32, i32), Vec<u32>, BuildHasherDefault<CellHasher>>;

/// One entity as the grid holds it.
#[derive(Debug, Clone, Copy)]
struct Entry {
    id: NetId,
    at: Vec2,
    radius: f32,
}

/// The replicated entities by cell, for one snapshot's relevance queries.
#[derive(Debug, Clone)]
pub struct InterestGrid {
    interest: Interest,
    /// Kept across rebuilds with their lists emptied, so a world whose
    /// entities stay in their cells allocates nothing.
    cells: Cells,
    /// The cells holding an entry since the last clear.
    occupied: usize,
    entries: Vec<Entry>,
    /// Entries relevant further than the session's radius: no cell range
    /// around a point covers them, so every query reads them all. Few in
    /// any sane game — a beacon, a boss.
    wide: Vec<u32>,
}

impl InterestGrid {
    /// An empty grid for `interest`, whose values are used as they are.
    pub fn new(interest: Interest) -> Self {
        Self {
            interest,
            cells: Cells::default(),
            occupied: 0,
            entries: Vec::new(),
            wide: Vec::new(),
        }
    }

    /// Empties it for the next snapshot.
    pub fn clear(&mut self) {
        if self.cells.len() > 4 * self.occupied + 64 {
            // Entities moved on: let go of the cells they left.
            self.cells.retain(|_, indices| !indices.is_empty());
        }
        for indices in self.cells.values_mut() {
            indices.clear();
        }
        self.occupied = 0;
        self.entries.clear();
        self.wide.clear();
    }

    /// How many entities it holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Puts `id` at `at`, relevant within `radius` — the session's for
    /// `None`, and for a radius that is not a positive number. An entity
    /// at a position that is not finite is never near anything, and is
    /// left out.
    pub fn insert(&mut self, id: NetId, at: Vec2, radius: Option<f32>) {
        if !at.is_finite() {
            return;
        }
        let radius = radius
            .filter(|r| r.is_finite() && *r > 0.0)
            .unwrap_or(self.interest.radius);
        let index = self.entries.len() as u32;
        self.entries.push(Entry { id, at, radius });
        if radius > self.interest.radius {
            self.wide.push(index);
        } else {
            let indices = self.cells.entry(self.cell(at)).or_default();
            if indices.is_empty() {
                self.occupied += 1;
            }
            indices.push(index);
        }
    }

    /// Adds to `out` every entity relevant to a client watching `points`,
    /// given the ones relevant to it before (`previous`): one not relevant
    /// before is within its radius of a point, one relevant before is
    /// within its leave radius ([`Interest::leave_radius`]) of one.
    pub fn relevant(&self, points: &[Vec2], previous: &HashSet<NetId>, out: &mut HashSet<NetId>) {
        let reach = self.interest.leave_radius(self.interest.radius);
        let check = |index: u32, point: Vec2, out: &mut HashSet<NetId>| {
            let entry = self.entries[index as usize];
            let distance = (entry.at - point).length_squared();
            let enter = entry.radius * entry.radius;
            let leave = self.interest.leave_radius(entry.radius).powi(2);
            if distance <= enter || (distance <= leave && previous.contains(&entry.id)) {
                out.insert(entry.id);
            }
        };
        for &point in points.iter().filter(|p| p.is_finite()) {
            let (x0, y0) = self.cell(point - Vec2::splat(reach));
            let (x1, y1) = self.cell(point + Vec2::splat(reach));
            // In i128: a range clamped to the whole of i32 both ways is
            // 2^64 cells, past i64 — an unchecked interest (a radius near
            // `f32::MAX`, a cell far below it) must not overflow here.
            let span =
                (i128::from(x1) - i128::from(x0) + 1) * (i128::from(y1) - i128::from(y0) + 1);
            if span > self.occupied as i128 {
                // A cell size far below the radius: fewer cells are
                // occupied than the range holds, so read those instead.
                for indices in self.cells.values() {
                    for &index in indices {
                        check(index, point, out);
                    }
                }
            } else {
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        for &index in self.cells.get(&(x, y)).into_iter().flatten() {
                            check(index, point, out);
                        }
                    }
                }
            }
            for &index in &self.wide {
                check(index, point, out);
            }
        }
    }

    fn cell(&self, at: Vec2) -> (i32, i32) {
        let cell = (at / self.interest.cell_size).floor();
        // `as` saturates: a point a world away lands in the last cell.
        (cell.x as i32, cell.y as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(entities: &[(NetId, Vec2)]) -> InterestGrid {
        let mut grid = InterestGrid::new(Interest::new(10.0));
        for &(id, at) in entities {
            grid.insert(id, at, None);
        }
        grid
    }

    fn query(grid: &InterestGrid, points: &[Vec2], previous: &[NetId]) -> Vec<NetId> {
        let previous: HashSet<NetId> = previous.iter().copied().collect();
        let mut out = HashSet::new();
        grid.relevant(points, &previous, &mut out);
        let mut out: Vec<NetId> = out.into_iter().collect();
        out.sort_unstable();
        out
    }

    #[test]
    fn what_is_within_the_radius_of_any_point_and_nothing_else() {
        let grid = grid(&[
            (1, Vec2::new(5.0, 0.0)),
            (2, Vec2::new(-9.9, 0.0)),
            (3, Vec2::new(25.0, 0.0)),
            (4, Vec2::new(-200.0, -200.0)),
            (5, Vec2::new(100.0, 4.0)),
        ]);
        assert_eq!(query(&grid, &[Vec2::ZERO], &[]), [1, 2]);
        assert_eq!(
            query(&grid, &[Vec2::ZERO, Vec2::new(100.0, 0.0)], &[]),
            [1, 2, 5]
        );
        assert!(query(&grid, &[], &[1, 2]).is_empty(), "no point, nothing");
    }

    #[test]
    fn a_relevant_entity_stays_until_past_the_leave_radius() {
        let grid = grid(&[(1, Vec2::new(10.5, 0.0)), (2, Vec2::new(11.5, 0.0))]);
        assert!(
            query(&grid, &[Vec2::ZERO], &[]).is_empty(),
            "outside: not entered"
        );
        assert_eq!(query(&grid, &[Vec2::ZERO], &[1, 2]), [1], "1 kept, 2 left");
    }

    #[test]
    fn its_own_radius_reaches_past_the_cells_around_a_point() {
        let mut grid = grid(&[]);
        grid.insert(1, Vec2::new(80.0, 0.0), Some(100.0));
        grid.insert(2, Vec2::new(3.0, 0.0), Some(2.0));
        grid.insert(3, Vec2::new(1.0, 0.0), Some(f32::NAN));
        grid.insert(4, Vec2::new(f32::INFINITY, 0.0), None);
        assert_eq!(grid.len(), 3);
        assert_eq!(query(&grid, &[Vec2::ZERO], &[]), [1, 3]);
    }

    #[test]
    fn a_tiny_cell_reads_the_occupied_cells_and_answers_the_same() {
        let mut fine = InterestGrid::new(Interest {
            cell_size: 0.001,
            ..Interest::new(10.0)
        });
        for (id, at) in [(1, Vec2::new(5.0, 5.0)), (2, Vec2::new(-30.0, 2.0))] {
            fine.insert(id, at, None);
        }
        assert_eq!(query(&fine, &[Vec2::ZERO], &[]), [1]);
    }

    #[test]
    fn negative_coordinates_fall_in_their_own_cells() {
        let grid = grid(&[(1, Vec2::new(-0.5, -0.5)), (2, Vec2::new(-19.0, -1.0))]);
        assert_eq!(query(&grid, &[Vec2::new(-10.0, 0.0)], &[]), [1, 2]);
    }

    #[test]
    fn a_rebuild_forgets_where_entities_were() {
        let mut grid = grid(&[(1, Vec2::new(1.0, 0.0))]);
        for step in 0..200 {
            grid.clear();
            grid.insert(1, Vec2::new(step as f32 * 15.0, 0.0), None);
        }
        assert_eq!(grid.len(), 1);
        assert!(query(&grid, &[Vec2::ZERO], &[]).is_empty());
        assert_eq!(query(&grid, &[Vec2::new(2985.0, 0.0)], &[]), [1]);
        assert!(grid.cells.len() < 200, "left cells let go of");
    }

    #[test]
    fn extreme_values_unchecked_answer_without_overflow_or_a_long_walk() {
        for interest in [
            Interest::new(f32::MAX),
            Interest {
                hysteresis: f32::MAX,
                ..Interest::new(10.0)
            },
            Interest {
                cell_size: 1.0e-3,
                ..Interest::new(1.0e7)
            },
        ] {
            let mut grid = InterestGrid::new(interest);
            grid.insert(1, Vec2::new(5.0, 0.0), None);
            grid.insert(2, Vec2::new(-1.0e6, 3.0e6), None);
            let started = std::time::Instant::now();
            let found = query(&grid, &[Vec2::ZERO], &[1, 2]);
            assert!(found.contains(&1), "{interest:?}: {found:?}");
            assert!(started.elapsed().as_secs() < 1, "{interest:?}");
        }
        for interest in [
            Interest::new(f32::MAX),
            Interest {
                cell_size: 1.0e-3,
                ..Interest::new(1.0e7)
            },
        ] {
            let Some(checked) = interest.checked() else {
                continue;
            };
            let mut grid = InterestGrid::new(checked);
            grid.insert(1, Vec2::new(9.0e6, 0.0), None);
            assert_eq!(query(&grid, &[Vec2::ZERO], &[]), [1]);
        }
    }
}
