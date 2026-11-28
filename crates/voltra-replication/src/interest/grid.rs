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
mod tests;
