//! A grid baked from a solid tilemap layer, through the colliders the
//! layer generates.

use voltra_ecs::World;
use voltra_nav::{NavGrid, NavSettings};
use voltra_render::glam::{IVec2, Vec2};
use voltra_scene::tilemap::sync_colliders;
use voltra_scene::{Tile, Tilemap, Transform2D};

#[test]
fn a_solid_layer_blocks_its_tiles_and_a_path_goes_round_them() {
    let mut world = World::new();
    let layer = world.spawn();
    world.insert(layer, Transform2D::default());
    let mut map = Tilemap::new(Vec2::ONE);
    map.collision = true;
    // A wall of tiles up column 3, rows 0..6, in an 8×8 room.
    for y in 0..6 {
        map.set(IVec2::new(3, y), Some(Tile::new(0)));
    }
    world.insert(layer, map);
    sync_colliders(&mut world);

    let settings = NavSettings {
        cell_size: 0.5,
        clearance: Vec2::splat(0.2),
        ..NavSettings::default()
    };
    let grid = NavGrid::bake(&world, Vec2::ZERO, Vec2::splat(8.0), &settings);
    let blocked = grid.cell_of(Vec2::new(3.5, 2.5)).expect("on the grid");
    assert!(grid.is_blocked(blocked));
    assert!(!grid.is_blocked(grid.cell_of(Vec2::new(3.5, 7.0)).expect("on the grid")));

    let path = grid
        .find_path(Vec2::new(1.0, 1.0), Vec2::new(6.0, 1.0))
        .expect("over the top of the wall");
    assert!(
        path.points().iter().any(|p| p.y > 6.0),
        "round the end: {:?}",
        path.points()
    );
}
