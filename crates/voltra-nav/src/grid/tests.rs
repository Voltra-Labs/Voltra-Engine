//! Grids baked from a few walls, and the paths through them.

use voltra_ecs::World;
use voltra_render::glam::{IVec2, Vec2};
use voltra_scene::{Collider, OneWay, RigidBody, Transform};

use super::*;
use crate::Path;

fn wall(world: &mut World, at: Vec2, half: Vec2) {
    let entity = world.spawn();
    world.insert(entity, Transform::from_translation(at));
    world.insert(entity, Collider::Box { half_extents: half });
}

fn settings(clearance: f32) -> NavSettings {
    NavSettings {
        cell_size: 0.5,
        clearance: Vec2::splat(clearance),
        ..NavSettings::default()
    }
}

/// A 10×10 room from the origin with a wall across the middle, open at the
/// right end.
fn room() -> (World, NavGrid) {
    let mut world = World::new();
    wall(&mut world, Vec2::new(4.0, 5.0), Vec2::new(4.0, 0.25));
    let grid = NavGrid::bake(&world, Vec2::ZERO, Vec2::splat(10.0), &settings(0.0));
    (world, grid)
}

#[test]
fn a_wall_blocks_the_cells_it_covers_and_not_those_it_touches() {
    let (_, grid) = room();
    assert_eq!(grid.size(), IVec2::new(20, 20));
    assert!(grid.is_blocked(IVec2::new(4, 10)), "under the wall");
    assert!(!grid.is_blocked(IVec2::new(4, 11)), "above it");
    assert!(
        !grid.is_blocked(IVec2::new(4, 8)),
        "beside it, not touching"
    );
    assert!(!grid.is_blocked(IVec2::new(16, 10)), "past its end");
    assert!(grid.is_blocked(IVec2::new(-1, 0)), "off the grid");
}

#[test]
fn clearance_grows_the_walls_by_the_agent() {
    let mut world = World::new();
    wall(&mut world, Vec2::new(4.0, 5.0), Vec2::new(4.0, 0.25));
    let wide = NavGrid::bake(&world, Vec2::ZERO, Vec2::splat(10.0), &settings(0.4));
    assert!(
        wide.is_blocked(IVec2::new(4, 11)),
        "too close for the agent"
    );
    assert!(!wide.is_blocked(IVec2::new(4, 12)));
}

#[test]
fn a_path_goes_round_the_wall_and_is_straightened() {
    let (_, grid) = room();
    let mut path = grid
        .find_path(Vec2::new(1.0, 1.0), Vec2::new(1.0, 9.0))
        .expect("a way round");
    assert_eq!(path.points().first(), Some(&Vec2::new(1.0, 1.0)));
    assert_eq!(path.end(), Some(Vec2::new(1.0, 9.0)), "ends where asked");
    assert!(
        path.points().len() <= 5,
        "straightened: {:?}",
        path.points()
    );
    assert!(
        path.points().iter().any(|p| p.x > 8.0),
        "round the open end"
    );
    for pair in path.points().windows(2) {
        assert!(
            grid.line_of_sight(pair[0], pair[1]),
            "every leg is clear: {pair:?} of {:?}",
            path.points()
        );
    }
    assert!(path.length() > 16.0);
    let first = path
        .next_point(Vec2::new(1.0, 1.0), 0.1)
        .expect("a waypoint");
    assert_ne!(first, Vec2::new(1.0, 1.0));
}

#[test]
fn an_unreachable_goal_has_no_path_but_a_closest_one() {
    let mut world = World::new();
    // A closed box around (5, 5).
    wall(&mut world, Vec2::new(5.0, 7.0), Vec2::new(2.0, 0.25));
    wall(&mut world, Vec2::new(5.0, 3.0), Vec2::new(2.0, 0.25));
    wall(&mut world, Vec2::new(3.0, 5.0), Vec2::new(0.25, 2.0));
    wall(&mut world, Vec2::new(7.0, 5.0), Vec2::new(0.25, 2.0));
    let grid = NavGrid::bake(&world, Vec2::ZERO, Vec2::splat(10.0), &settings(0.0));
    assert!(grid
        .find_path(Vec2::new(1.0, 1.0), Vec2::new(5.0, 5.0))
        .is_none());
    let closest = grid
        .find_path_closest(Vec2::new(1.0, 1.0), Vec2::new(5.0, 5.0))
        .expect("as near as it gets");
    let end = closest.end().expect("an end");
    assert!(
        end.distance(Vec2::new(5.0, 5.0)) < 3.0,
        "against the box: {end}"
    );
}

#[test]
fn diagonals_never_cut_a_blocked_corner() {
    let mut grid = NavGrid::new(Vec2::ZERO, 1.0, IVec2::new(3, 3));
    grid.set_blocked(IVec2::new(1, 0), true);
    grid.set_blocked(IVec2::new(0, 1), true);
    assert!(
        grid.find_path(Vec2::new(0.5, 0.5), Vec2::new(1.5, 1.5))
            .is_none(),
        "boxed in diagonally"
    );
    assert!(!grid.line_of_sight(Vec2::new(0.5, 0.5), Vec2::new(1.5, 1.5)));
}

#[test]
fn moving_bodies_characters_and_one_way_platforms_are_not_baked_in() {
    let mut world = World::new();
    let walker = world.spawn();
    world.insert(walker, Transform::from_translation(Vec2::new(4.0, 4.0)));
    world.insert(
        walker,
        Collider::Box {
            half_extents: Vec2::splat(0.5),
        },
    );
    world.insert(walker, voltra_scene::CharacterBody2D::default());
    let crate_body = world.spawn();
    world.insert(crate_body, Transform::from_translation(Vec2::new(2.0, 2.0)));
    world.insert(
        crate_body,
        Collider::Box {
            half_extents: Vec2::splat(0.5),
        },
    );
    world.insert(crate_body, RigidBody::new_dynamic(1.0));
    let ledge = world.spawn();
    world.insert(ledge, Transform::from_translation(Vec2::new(6.0, 6.0)));
    world.insert(
        ledge,
        Collider::Box {
            half_extents: Vec2::splat(0.5),
        },
    );
    world.insert(ledge, OneWay::default());
    let grid = NavGrid::bake(&world, Vec2::ZERO, Vec2::splat(10.0), &settings(0.0));
    assert_eq!(grid.blocked_count(), 0);
}

#[test]
fn bake_all_covers_every_obstacle_and_an_empty_world_is_an_empty_grid() {
    let (world, _) = room();
    let grid = NavGrid::bake_all(&world, &settings(0.0));
    assert!(grid.blocked_count() > 0);
    assert!(grid.cell_of(Vec2::new(0.0, 5.0)).is_some());
    let empty = NavGrid::bake_all(&World::new(), &settings(0.0));
    assert_eq!(empty.size(), IVec2::ZERO);
    assert!(empty.find_path(Vec2::ZERO, Vec2::ONE).is_none());
}

#[test]
fn a_start_pressed_into_a_wall_starts_from_the_nearest_free_cell() {
    let (_, grid) = room();
    let path = grid
        .find_path(Vec2::new(2.0, 5.1), Vec2::new(2.0, 8.0))
        .expect("from just inside");
    assert!(path.points().len() >= 2);
}

#[test]
fn a_path_hands_out_its_waypoints_in_turn() {
    let mut path = Path::new(vec![Vec2::ZERO, Vec2::X, Vec2::new(1.0, 1.0)]);
    assert_eq!(path.next_point(Vec2::ZERO, 0.1), Some(Vec2::X));
    assert_eq!(
        path.next_point(Vec2::new(0.95, 0.0), 0.1),
        Some(Vec2::new(1.0, 1.0))
    );
    assert_eq!(path.next_point(Vec2::new(1.0, 0.99), 0.1), None);
    assert!(path.is_finished());
}
