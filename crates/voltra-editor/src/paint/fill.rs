//! The bucket: every cell joined to the clicked one that holds what it holds.
//!
//! Four-connected, as Tiled's, Unity's and every paint program's fill is: a
//! diagonal gap is a wall, so a closed outline of tiles drawn corner to
//! corner still holds its fill in. A map here has no edge, so a fill of
//! empty cells is kept to the layer's extent — the box around every tile it
//! has — and to [`MAX_FILL`] cells: an empty start outside that box has
//! nothing to fill and fills nothing.

use std::collections::{HashSet, VecDeque};

use voltra_render::glam::IVec2;
use voltra_scene::Tilemap;

use super::stroke::MAX_FILL;

/// The cells a fill from `start` reaches, in the order reached; empty when
/// there is nothing to fill or more than [`MAX_FILL`] cells would be.
pub fn region(map: &Tilemap, start: IVec2) -> Vec<IVec2> {
    let target = map.get(start);
    let bounds = match (target, map.bounds()) {
        (_, None) => return Vec::new(),
        (Some(_), Some(bounds)) => bounds,
        (None, Some((min, max))) => {
            if start.cmplt(min).any() || start.cmpgt(max).any() {
                log::info!("nothing around ({}, {}) to fill", start.x, start.y);
                return Vec::new();
            }
            (min, max)
        }
    };
    let (min, max) = bounds;

    let mut seen = HashSet::from([start]);
    let mut queue = VecDeque::from([start]);
    let mut reached = Vec::new();
    while let Some(cell) = queue.pop_front() {
        reached.push(cell);
        if reached.len() as i64 > MAX_FILL {
            log::warn!("a fill of more than {MAX_FILL} cells; nothing filled");
            return Vec::new();
        }
        for step in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
            let next = cell + step;
            let inside = next.cmpge(min).all() && next.cmple(max).all();
            if inside && map.get(next) == target && seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    reached
}

#[cfg(test)]
mod tests {
    use voltra_render::glam::Vec2;
    use voltra_scene::Tile;

    use super::*;

    /// A ring of frame 1 around a 3×2 hole, with one more tile outside.
    fn ringed() -> Tilemap {
        let mut map = Tilemap::new(Vec2::ONE);
        for x in 0..5 {
            map.set(IVec2::new(x, 0), Some(Tile::new(1)));
            map.set(IVec2::new(x, 3), Some(Tile::new(1)));
        }
        for y in 1..3 {
            map.set(IVec2::new(0, y), Some(Tile::new(1)));
            map.set(IVec2::new(4, y), Some(Tile::new(1)));
        }
        map.set(IVec2::new(8, 8), Some(Tile::new(2)));
        map
    }

    #[test]
    fn an_enclosed_hole_fills_and_stops_at_its_walls() {
        let map = ringed();
        let mut cells = region(&map, IVec2::new(2, 1));
        cells.sort_by_key(|c| (c.y, c.x));
        let expected: Vec<IVec2> = (1..3)
            .flat_map(|y| (1..4).map(move |x| IVec2::new(x, y)))
            .collect();
        assert_eq!(cells, expected);
    }

    #[test]
    fn a_fill_on_a_tile_takes_the_tiles_joined_to_it() {
        let map = ringed();
        assert_eq!(region(&map, IVec2::new(0, 0)).len(), 14, "the whole ring");
        assert_eq!(region(&map, IVec2::new(8, 8)).len(), 1);
    }

    #[test]
    fn an_empty_cell_outside_every_tile_fills_nothing() {
        let map = ringed();
        assert!(region(&map, IVec2::new(-5, 2)).is_empty());
        assert!(region(&Tilemap::new(Vec2::ONE), IVec2::ZERO).is_empty());
    }

    #[test]
    fn an_open_area_fills_to_the_layers_extent() {
        let map = ringed();
        // Outside the ring but inside the box around all tiles: everything
        // empty and reachable up to (8, 8).
        let cells = region(&map, IVec2::new(6, 1));
        assert!(cells.iter().all(|c| c.cmple(IVec2::splat(8)).all()));
        assert!(!cells.contains(&IVec2::new(2, 1)), "not into the ring");
    }
}
