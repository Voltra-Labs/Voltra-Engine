//! Between the pointer's world position and a layer's cells.
//!
//! Through the layer's composed matrix, so a layer that is moved, turned,
//! scaled or parented is painted where it is drawn — the same rule picking
//! follows.

use voltra_ecs::{Entity, World};
use voltra_render::glam::{IVec2, Vec2};
use voltra_scene::hierarchy::world_matrix_2d;
use voltra_scene::Tilemap;

/// Below this the layer's matrix is treated as collapsed; matches the
/// picking threshold for the same reason (see `voltra_scene::pick`).
const MIN_DETERMINANT: f32 = 1e-12;

/// The cell of `layer` under the world point `at`, or `None` when the entity
/// is not a layer, is collapsed to nothing, or has sizeless tiles.
pub fn cell_under(world: &World, layer: Entity, at: Vec2) -> Option<IVec2> {
    let map = world.get::<Tilemap>(layer)?;
    let matrix = world_matrix_2d(world, layer);
    if matrix.determinant().abs() < MIN_DETERMINANT {
        return None;
    }
    map.cell_at(matrix.inverse().transform_point2(at))
}

/// The world-space corners of the cells from `a` to `b` (both included, in
/// any order), counter-clockwise from the lower-left — what an outline of a
/// cell, or of a rectangle of them, is drawn through.
pub fn corners(world: &World, layer: Entity, a: IVec2, b: IVec2) -> Option<[Vec2; 4]> {
    let map = world.get::<Tilemap>(layer)?;
    let matrix = world_matrix_2d(world, layer);
    let min = map.cell_origin(a.min(b));
    let max = map.cell_origin(a.max(b) + IVec2::ONE);
    Some(
        [
            Vec2::new(min.x, min.y),
            Vec2::new(max.x, min.y),
            Vec2::new(max.x, max.y),
            Vec2::new(min.x, max.y),
        ]
        .map(|corner| matrix.transform_point2(corner)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_scene::Transform2D;

    fn layer(world: &mut World, transform: Transform2D, tile: Vec2) -> Entity {
        let entity = world.spawn();
        world.insert(entity, transform);
        world.insert(entity, Tilemap::new(tile));
        entity
    }

    #[test]
    fn a_point_on_a_moved_layer_lands_in_the_cell_drawn_there() {
        let mut world = World::new();
        let entity = layer(
            &mut world,
            Transform2D::from_translation(Vec2::new(10.0, 5.0)),
            Vec2::splat(0.5),
        );

        assert_eq!(
            cell_under(&world, entity, Vec2::new(10.75, 5.25)),
            Some(IVec2::new(1, 0))
        );
        assert_eq!(
            cell_under(&world, entity, Vec2::new(9.9, 4.9)),
            Some(IVec2::new(-1, -1))
        );
    }

    #[test]
    fn a_turned_layer_is_painted_in_its_own_axes() {
        let mut world = World::new();
        let entity = layer(
            &mut world,
            Transform2D::default().with_rotation(std::f32::consts::FRAC_PI_2),
            Vec2::ONE,
        );
        // A quarter turn puts the layer's +x along the world's +y.
        assert_eq!(
            cell_under(&world, entity, Vec2::new(-0.5, 2.5)),
            Some(IVec2::new(2, 0))
        );
    }

    #[test]
    fn a_collapsed_layer_has_no_cell_anywhere() {
        let mut world = World::new();
        let entity = layer(
            &mut world,
            Transform2D::default().with_scale(Vec2::new(0.0, 1.0)),
            Vec2::ONE,
        );
        assert_eq!(cell_under(&world, entity, Vec2::ZERO), None);
    }

    #[test]
    fn something_that_is_not_a_layer_has_no_cells() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform2D::default());
        assert_eq!(cell_under(&world, entity, Vec2::ZERO), None);
        assert_eq!(corners(&world, entity, IVec2::ZERO, IVec2::ZERO), None);
    }

    #[test]
    fn the_corners_cover_both_end_cells_in_either_order() {
        let mut world = World::new();
        let entity = layer(&mut world, Transform2D::default(), Vec2::splat(2.0));

        let forward = corners(&world, entity, IVec2::new(-1, 0), IVec2::new(1, 2));
        let backward = corners(&world, entity, IVec2::new(1, 2), IVec2::new(-1, 0));

        assert_eq!(forward, backward);
        let [low, _, high, _] = forward.expect("a layer");
        assert_eq!(low, Vec2::new(-2.0, 0.0));
        assert_eq!(high, Vec2::new(4.0, 6.0));
    }
}
