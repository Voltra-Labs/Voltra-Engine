//! A level's tiles asked about by a point in the world: which cell is under
//! it, and whether a tile there says something — `ladder`, `water`, `damage`.

use voltra_ecs::{Entity, World};
use voltra_render::glam::{IVec2, Vec2};

use super::{TileValue, Tilemap};
use crate::hierarchy::world_matrix_2d;

/// Below this the layer's matrix is treated as collapsed, as picking does.
const MIN_DETERMINANT: f32 = 1e-12;

/// The cell of `layer` under the world point `at`, through the layer's
/// transform and its parents'. `None` when `layer` is no layer, or is
/// collapsed to nothing.
pub fn cell_under(world: &World, layer: Entity, at: Vec2) -> Option<IVec2> {
    let map = world.get::<Tilemap>(layer)?;
    let matrix = world_matrix_2d(world, layer);
    if matrix.determinant().abs() < MIN_DETERMINANT {
        return None;
    }
    map.cell_at(matrix.inverse().transform_point2(at))
}

/// The first layer whose tile under the world point `at` has the custom
/// value `key`, with the cell and the value: what a game asks each tick of
/// the point its character stands at.
pub fn custom_under<'w>(
    world: &'w World,
    at: Vec2,
    key: &str,
) -> Option<(Entity, IVec2, &'w TileValue)> {
    world.query::<Tilemap>().find_map(|(layer, map)| {
        let cell = cell_under(world, layer, at)?;
        Some((layer, cell, map.custom_at(cell, key)?))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Tile, Transform2D};

    #[test]
    fn a_point_finds_the_ladder_tile_it_is_on() {
        let mut world = World::new();
        let mut map = Tilemap::new(Vec2::splat(0.5));
        map.set(IVec2::new(1, 0), Some(Tile::new(3)));
        map.tile_data
            .entry(3)
            .or_default()
            .custom
            .insert("ladder".into(), TileValue::Bool(true));
        let layer = world.spawn();
        world.insert(layer, Transform2D::from_translation(Vec2::new(10.0, 0.0)));
        world.insert(layer, map);

        let found = custom_under(&world, Vec2::new(10.7, 0.2), "ladder");
        assert_eq!(
            found.map(|(l, c, _)| (l, c)),
            Some((layer, IVec2::new(1, 0)))
        );
        assert!(custom_under(&world, Vec2::new(10.2, 0.2), "ladder").is_none());
    }
}
