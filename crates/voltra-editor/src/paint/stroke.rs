//! What one stroke does to a layer's cells.
//!
//! Kept apart from the pointer handling so the edits themselves are plain
//! functions on a [`Tilemap`] and testable without egui.

use voltra_render::glam::IVec2;
use voltra_scene::{Tile, Tilemap};

use super::line::cells_between;
use super::stamp::Stamp;

/// The most cells one rectangle fill writes.
///
/// A rectangle dragged across a zoomed-out view can name millions of cells,
/// and writing them all freezes the editor for seconds on a mis-drag. A
/// million is far past any level's layer (Tiled's own default map is 100×100)
/// and still a fill that finishes inside a frame or two.
pub const MAX_FILL: i64 = 1 << 20;

/// Writes `tile` — or empties, for `None` — every cell on the line from
/// `from` to `to`.
pub fn line(map: &mut Tilemap, from: IVec2, to: IVec2, tile: Option<Tile>) {
    for cell in cells_between(from, to) {
        map.set(cell, tile);
    }
}

/// Paints `stamp` with its bottom-left on every cell of the line from `from`
/// to `to`: a one-tile stamp is [`line`], a block overlaps itself along the
/// way, as a dragged stamp does in Tiled.
pub fn line_stamp(map: &mut Tilemap, from: IVec2, to: IVec2, stamp: &Stamp) {
    for cell in cells_between(from, to) {
        stamp.paint(map, cell);
    }
}

/// Fills the rectangle with corners `a` and `b` with `stamp` repeated from
/// its lower-left corner, holes left alone. Returns whether it did: a
/// rectangle over [`MAX_FILL`] cells is refused whole and logged, never
/// half-filled.
pub fn fill_stamp(map: &mut Tilemap, a: IVec2, b: IVec2, stamp: &Stamp) -> bool {
    let min = a.min(b);
    let max = a.max(b);
    let size = max.as_i64vec2() - min.as_i64vec2() + 1;
    if size.x.saturating_mul(size.y) > MAX_FILL {
        log::warn!(
            "a {}×{} fill is beyond the {MAX_FILL} cells one fill may write; nothing filled",
            size.x,
            size.y
        );
        return false;
    }
    for y in min.y..=max.y {
        for x in min.x..=max.x {
            let cell = IVec2::new(x, y);
            if let Some(tile) = stamp.pattern(min, cell) {
                map.set(cell, Some(tile));
            }
        }
    }
    true
}

/// [`line`] for a terrain: every cell on the line painted with terrain
/// `terrain` — or emptied, for `None` — and its edges re-picked.
pub fn line_terrain(map: &mut Tilemap, from: IVec2, to: IVec2, terrain: Option<usize>) {
    for cell in cells_between(from, to) {
        map.paint_terrain(cell, terrain);
    }
}

/// [`fill_stamp`] for a terrain, under the same [`MAX_FILL`] limit.
pub fn fill_terrain(map: &mut Tilemap, a: IVec2, b: IVec2, terrain: Option<usize>) -> bool {
    let min = a.min(b);
    let max = a.max(b);
    let size = max.as_i64vec2() - min.as_i64vec2() + 1;
    if size.x.saturating_mul(size.y) > MAX_FILL {
        log::warn!(
            "a {}×{} fill is beyond the {MAX_FILL} cells one fill may write; nothing filled",
            size.x,
            size.y
        );
        return false;
    }
    for y in min.y..=max.y {
        for x in min.x..=max.x {
            map.paint_terrain(IVec2::new(x, y), terrain);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use voltra_render::glam::Vec2;

    use super::*;

    #[test]
    fn a_line_paints_every_cell_it_crosses() {
        let mut map = Tilemap::new(Vec2::ONE);
        line(
            &mut map,
            IVec2::new(0, 0),
            IVec2::new(4, 0),
            Some(Tile::new(2)),
        );

        assert_eq!(map.len(), 5);
        assert_eq!(map.get(IVec2::new(2, 0)), Some(Tile::new(2)));
    }

    #[test]
    fn erasing_a_line_empties_it() {
        let mut map = Tilemap::new(Vec2::ONE);
        fill_stamp(
            &mut map,
            IVec2::ZERO,
            IVec2::new(4, 1),
            &Stamp::single(Tile::new(0)),
        );
        line(&mut map, IVec2::new(0, 1), IVec2::new(4, 1), None);

        assert_eq!(map.len(), 5);
        assert_eq!(map.get(IVec2::new(3, 1)), None);
    }

    #[test]
    fn a_fill_covers_the_rectangle_from_either_corner() {
        let mut map = Tilemap::new(Vec2::ONE);
        assert!(fill_stamp(
            &mut map,
            IVec2::new(2, 1),
            IVec2::new(-1, -1),
            &Stamp::single(Tile::new(1))
        ));

        assert_eq!(map.len(), 12);
        assert_eq!(map.bounds(), Some((IVec2::new(-1, -1), IVec2::new(2, 1))));
    }

    #[test]
    fn an_enormous_fill_is_refused_whole() {
        let mut map = Tilemap::new(Vec2::ONE);
        let filled = fill_stamp(
            &mut map,
            IVec2::ZERO,
            IVec2::new(4096, 4096),
            &Stamp::single(Tile::new(0)),
        );

        assert!(!filled);
        assert!(map.is_empty());
    }

    #[test]
    fn a_fill_at_the_edges_of_the_grid_does_not_overflow() {
        let mut map = Tilemap::new(Vec2::ONE);
        assert!(!fill_stamp(
            &mut map,
            IVec2::MIN,
            IVec2::MAX,
            &Stamp::single(Tile::new(0))
        ));
    }

    #[test]
    fn a_terrain_line_joins_its_cells_up() {
        use voltra_scene::tilemap::Terrain;

        let mut map = Tilemap::new(Vec2::ONE);
        map.terrains.push(Terrain {
            name: "grass".to_owned(),
            frames: std::array::from_fn(|mask| 100 + mask as u32),
            blob: Vec::new(),
        });
        line_terrain(&mut map, IVec2::new(0, 0), IVec2::new(2, 0), Some(0));
        let frames: Vec<u32> = (0..3)
            .map(|x| map.get(IVec2::new(x, 0)).map_or(0, |t| t.frame))
            .collect();
        // East only, east and west, west only.
        assert_eq!(frames, vec![102, 110, 108]);
    }

    #[test]
    fn a_terrain_fill_is_bounded_like_any_fill() {
        let mut map = Tilemap::new(Vec2::ONE);
        map.terrains
            .push(voltra_scene::tilemap::Terrain::uniform("g", 1));
        assert!(!fill_terrain(
            &mut map,
            IVec2::ZERO,
            IVec2::splat(5_000),
            Some(0)
        ));
        assert!(map.is_empty());
    }
}
