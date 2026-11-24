//! The fewest boxes that cover a layer's tiles, near enough.
//!
//! One collider per tile is what a naive tilemap collider does, and it is
//! wrong twice: a 100-tile floor is 100 bodies for the broad phase, and a
//! body sliding along it catches on every seam between two boxes whose edges
//! meet (the "ghost collision" every Box2D platformer FAQ warns about).
//! Unity's `CompositeCollider2D` and Godot's tile physics both merge first.
//!
//! **Greedy, not optimal.** Each row is cut into runs of filled cells, and a
//! run grows upward for as long as the row above holds exactly the same run.
//! The minimum rectangle cover is a harder problem than a level needs solved;
//! this one is linear in the tiles, deterministic, and gives a solid block
//! one box whatever its size, which is the case that matters.

use std::collections::BTreeMap;

use voltra_render::glam::IVec2;

use super::Tilemap;

/// A rectangle of cells: `min` included, `max` excluded on both axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellRect {
    /// The first cell, included.
    pub min: IVec2,
    /// One past the last cell, excluded.
    pub max: IVec2,
}

impl CellRect {
    /// How many cells across and up.
    pub fn size(&self) -> IVec2 {
        self.max - self.min
    }
}

/// Boxes covering every cell of `map` whose tile is [full](super::TileShape::Full)
/// exactly once, bottom row first and left to right within a row. Tiles of
/// other shapes are left to [`sync_colliders`](super::sync_colliders), one
/// polygon each.
pub fn solid_boxes(map: &Tilemap) -> Vec<CellRect> {
    let rows = rows(map);

    // Runs still growing, keyed by their `(start, end)` columns: a run on the
    // row above with the same key extends the box rather than starting one.
    let mut open: BTreeMap<(i32, i32), CellRect> = BTreeMap::new();
    let mut done = Vec::new();
    let mut previous_row = None;

    for (&y, columns) in &rows {
        // A gap between rows closes everything: a run cannot grow across a
        // row that holds nothing.
        if previous_row != Some(y - 1) {
            done.extend(std::mem::take(&mut open).into_values());
        }
        previous_row = Some(y);

        let mut next = BTreeMap::new();
        for run in runs(columns) {
            let rect = match open.remove(&run) {
                Some(mut rect) => {
                    rect.max.y = y + 1;
                    rect
                }
                None => CellRect {
                    min: IVec2::new(run.0, y),
                    max: IVec2::new(run.1, y + 1),
                },
            };
            next.insert(run, rect);
        }
        // Whatever the row above had that this one did not continue is closed.
        done.extend(std::mem::replace(&mut open, next).into_values());
    }
    done.extend(open.into_values());

    done.sort_by_key(|rect| (rect.min.y, rect.min.x));
    done
}

/// Every row's filled columns, sorted, rows in ascending order.
fn rows(map: &Tilemap) -> BTreeMap<i32, Vec<i32>> {
    let mut rows: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
    for (cell, tile) in map.tiles() {
        if map.shape_of(tile.frame).is_full() {
            rows.entry(cell.y).or_default().push(cell.x);
        }
    }
    for columns in rows.values_mut() {
        columns.sort_unstable();
    }
    rows
}

/// Sorted columns cut into `(start, end)` runs of consecutive ones, `end`
/// excluded.
fn runs(columns: &[i32]) -> Vec<(i32, i32)> {
    let mut runs: Vec<(i32, i32)> = Vec::new();
    for &x in columns {
        match runs.last_mut() {
            Some(run) if run.1 == x => run.1 = x + 1,
            _ => runs.push((x, x + 1)),
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tile;

    fn rect(min: (i32, i32), max: (i32, i32)) -> CellRect {
        CellRect {
            min: IVec2::new(min.0, min.1),
            max: IVec2::new(max.0, max.1),
        }
    }

    fn map(cells: &[(i32, i32)]) -> Tilemap {
        let mut map = Tilemap::default();
        for &(x, y) in cells {
            map.set(IVec2::new(x, y), Some(Tile::new(0)));
        }
        map
    }

    fn block(min: (i32, i32), max: (i32, i32)) -> Vec<(i32, i32)> {
        (min.1..max.1)
            .flat_map(|y| (min.0..max.0).map(move |x| (x, y)))
            .collect()
    }

    /// Every filled cell in exactly one box, and no box over an empty cell.
    fn assert_exact_cover(map: &Tilemap, boxes: &[CellRect]) {
        let mut covered = std::collections::HashMap::new();
        for b in boxes {
            for y in b.min.y..b.max.y {
                for x in b.min.x..b.max.x {
                    let cell = IVec2::new(x, y);
                    assert!(map.get(cell).is_some(), "{b:?} covers empty {cell}");
                    *covered.entry(cell).or_insert(0) += 1;
                }
            }
        }
        for (cell, _) in map.tiles() {
            assert_eq!(covered.get(&cell), Some(&1), "{cell} covered wrongly");
        }
    }

    #[test]
    fn an_empty_map_has_no_boxes() {
        assert!(solid_boxes(&Tilemap::default()).is_empty());
    }

    #[test]
    fn a_solid_block_is_one_box() {
        let map = map(&block((0, 0), (48, 3)));
        assert_eq!(solid_boxes(&map), vec![rect((0, 0), (48, 3))]);
    }

    #[test]
    fn a_block_across_chunk_borders_and_below_zero_is_still_one_box() {
        let map = map(&block((-20, -5), (20, 18)));
        assert_eq!(solid_boxes(&map), vec![rect((-20, -5), (20, 18))]);
    }

    #[test]
    fn a_gap_in_a_row_splits_it() {
        let map = map(&[(0, 0), (1, 0), (3, 0)]);
        assert_eq!(
            solid_boxes(&map),
            vec![rect((0, 0), (2, 1)), rect((3, 0), (4, 1))]
        );
    }

    #[test]
    fn an_empty_row_between_two_stops_the_growth() {
        let map = map(&[(0, 0), (0, 2)]);
        assert_eq!(
            solid_boxes(&map),
            vec![rect((0, 0), (1, 1)), rect((0, 2), (1, 3))]
        );
    }

    #[test]
    fn a_step_is_covered_exactly() {
        // An L: a floor three wide with a column on its left end.
        let mut cells = block((0, 0), (3, 1));
        cells.extend([(0, 1), (0, 2)]);
        let map = map(&cells);

        let boxes = solid_boxes(&map);

        assert_exact_cover(&map, &boxes);
        assert_eq!(boxes.len(), 2);
    }

    #[test]
    fn a_scatter_is_covered_exactly() {
        let cells: Vec<(i32, i32)> = (0..400)
            .filter(|i| (i * 7919) % 5 < 3)
            .map(|i| (i % 23 - 11, i / 23 - 8))
            .collect();
        let map = map(&cells);

        assert_exact_cover(&map, &solid_boxes(&map));
    }

    #[test]
    fn the_order_is_bottom_row_first_then_left_to_right() {
        let map = map(&[(5, 1), (0, 1), (2, 0)]);
        let starts: Vec<IVec2> = solid_boxes(&map).iter().map(|b| b.min).collect();
        assert_eq!(
            starts,
            vec![IVec2::new(2, 0), IVec2::new(0, 1), IVec2::new(5, 1)]
        );
    }
}
