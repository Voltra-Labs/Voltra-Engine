//! Which tilemap layer has a tile under a point.
//!
//! A layer is hit where it has a tile and nowhere else — Godot's rule for
//! clicking a `TileMapLayer`. Its bounds would be the cheaper test and the
//! wrong one: an L-shaped level's bounding box covers the sky above the short
//! leg, and a click there should reach whatever is drawn in the sky.

use voltra_ecs::{Entity, World};
use voltra_render::glam::{Mat3, Vec2};

use super::MIN_DETERMINANT;
use crate::hierarchy::WorldTransforms;
use crate::sprite::draw_key;
use crate::tilemap::Tilemap;

/// The topmost layer with a tile under `point`, in world space, and the key
/// it draws at — so a caller can weigh it against the sprite under the same
/// point.
///
/// `camera` is the view's position, for a layer under a [`Parallax`](crate::Parallax).
pub fn layer_at(
    world: &World,
    point: Vec2,
    sheets: crate::sprite::sheets::Sheets<'_>,
) -> Option<(Entity, (i32, i32, u32))> {
    let transforms = WorldTransforms::seen_from(world, sheets.camera);
    world
        .query::<Tilemap>()
        .filter(|(entity, map)| has_tile_at(transforms.matrix(*entity), point, map))
        .map(|(entity, map)| {
            let rank = sheets.rank(&map.sorting_layer);
            (entity, draw_key(entity, rank, map.sort_order))
        })
        .max_by_key(|(_, key)| *key)
}

/// Whether the layer `matrix` places has a tile under `point`.
fn has_tile_at(matrix: Mat3, point: Vec2, map: &Tilemap) -> bool {
    // See `MIN_DETERMINANT`: a collapsed layer is nowhere, not everywhere.
    if matrix.determinant().abs() < MIN_DETERMINANT {
        return false;
    }
    let local = matrix.inverse().transform_point2(point);
    map.cell_at(local)
        .is_some_and(|cell| map.get(cell).is_some())
}

#[cfg(test)]
mod tests {
    use voltra_render::glam::IVec2;

    use super::*;
    use crate::{Tile, Transform};

    fn layer(world: &mut World, at: Vec2, sort_order: i32, cells: &[(i32, i32)]) -> Entity {
        let mut map = Tilemap::new(Vec2::ONE);
        map.sort_order = sort_order;
        for &(x, y) in cells {
            map.set(IVec2::new(x, y), Some(Tile::new(0)));
        }
        let entity = world.spawn();
        world.insert(entity, Transform::from_translation(at));
        world.insert(entity, map);
        entity
    }

    fn hit(world: &World, point: Vec2) -> Option<Entity> {
        layer_at(world, point, crate::sprite::sheets::Sheets::default()).map(|(entity, _)| entity)
    }

    #[test]
    fn a_tile_under_the_point_is_a_hit() {
        let mut world = World::new();
        let entity = layer(&mut world, Vec2::new(10.0, 0.0), 0, &[(0, 0)]);

        assert_eq!(hit(&world, Vec2::new(10.5, 0.5)), Some(entity));
    }

    #[test]
    fn an_empty_cell_inside_the_bounds_is_a_miss() {
        let mut world = World::new();
        layer(&mut world, Vec2::ZERO, 0, &[(0, 0), (2, 2)]);

        assert_eq!(hit(&world, Vec2::new(1.5, 1.5)), None);
    }

    #[test]
    fn negative_cells_are_hit_where_they_are_drawn() {
        let mut world = World::new();
        let entity = layer(&mut world, Vec2::ZERO, 0, &[(-1, -1)]);

        assert_eq!(hit(&world, Vec2::new(-0.5, -0.5)), Some(entity));
        assert_eq!(hit(&world, Vec2::new(0.5, 0.5)), None);
    }

    #[test]
    fn the_layer_in_front_wins() {
        let mut world = World::new();
        let back = layer(&mut world, Vec2::ZERO, -1, &[(0, 0)]);
        let front = layer(&mut world, Vec2::ZERO, 3, &[(0, 0)]);

        assert_eq!(hit(&world, Vec2::splat(0.5)), Some(front));
        world.get_mut::<Tilemap>(front).expect("a layer").sort_order = -2;
        assert_eq!(hit(&world, Vec2::splat(0.5)), Some(back));
    }

    #[test]
    fn a_collapsed_layer_is_never_hit() {
        let mut world = World::new();
        let entity = layer(&mut world, Vec2::ZERO, 0, &[(0, 0)]);
        world.get_mut::<Transform>(entity).expect("a layer").scale = Vec2::ZERO;

        assert_eq!(hit(&world, Vec2::ZERO), None);
    }
}
