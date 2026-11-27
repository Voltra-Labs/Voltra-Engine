//! One-way platforms, for bodies the step moves and for characters, and
//! characters riding platforms that move.

mod common;

use common::*;
use voltra_ecs::{Entity, World};
use voltra_physics::{move_and_slide, PhysicsWorld};
use voltra_render::glam::Vec2;
use voltra_scene::{CharacterBody2D, Collider2D, OneWay, RigidBody2D, Transform2D};

/// A one-way platform, its top at y = 0, `half` wide.
fn platform(world: &mut World, half: f32) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform2D::from_translation(Vec2::new(0.0, -0.1)));
    world.insert(
        entity,
        Collider2D::Box {
            half_extents: Vec2::new(half, 0.1),
        },
    );
    world.insert(entity, OneWay::default());
    entity
}

fn crate_at(world: &mut World, at: Vec2, velocity: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform2D::from_translation(at));
    world.insert(
        entity,
        Collider2D::Box {
            half_extents: Vec2::splat(0.2),
        },
    );
    let mut body = RigidBody2D::new_dynamic(1.0);
    body.velocity = velocity;
    world.insert(entity, body);
    entity
}

fn character(world: &mut World, at: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform2D::from_translation(at));
    world.insert(
        entity,
        Collider2D::Box {
            half_extents: Vec2::splat(0.25),
        },
    );
    world.insert(entity, CharacterBody2D::default());
    entity
}

fn steps(world: &mut World, physics: &mut PhysicsWorld, count: usize) {
    for _ in 0..count {
        physics.step_once(world, G);
    }
}

#[test]
fn a_body_lands_on_a_one_way_platform_from_above() {
    let mut world = World::new();
    platform(&mut world, 2.0);
    let dropped = crate_at(&mut world, Vec2::new(0.0, 1.0), Vec2::ZERO);
    let mut physics = PhysicsWorld::new();
    steps(&mut world, &mut physics, 180);
    let y = translation(&world, dropped).y;
    assert!((y - 0.2).abs() < 0.03, "resting on top: {y}");
}

#[test]
fn a_body_thrown_up_from_below_passes_through_and_lands_on_top() {
    let mut world = World::new();
    platform(&mut world, 2.0);
    let thrown = crate_at(&mut world, Vec2::new(0.0, -1.0), Vec2::new(0.0, 7.0));
    let mut physics = PhysicsWorld::new();
    let mut highest = f32::NEG_INFINITY;
    for _ in 0..240 {
        physics.step_once(&mut world, G);
        highest = highest.max(translation(&world, thrown).y);
    }
    assert!(highest > 0.6, "through and clear: {highest}");
    let y = translation(&world, thrown).y;
    assert!((y - 0.2).abs() < 0.03, "and back down on top: {y}");
}

#[test]
fn a_solid_platform_still_stops_a_body_from_below() {
    let mut world = World::new();
    let solid = platform(&mut world, 2.0);
    world.remove::<OneWay>(solid);
    let thrown = crate_at(&mut world, Vec2::new(0.0, -1.0), Vec2::new(0.0, 7.0));
    let mut physics = PhysicsWorld::new();
    for _ in 0..60 {
        physics.step_once(&mut world, G);
        assert!(translation(&world, thrown).y < -0.1, "never through");
    }
}

#[test]
fn a_character_jumps_up_through_and_stands_on_top() {
    let mut world = World::new();
    let floor = world.spawn();
    world.insert(floor, Transform2D::from_translation(Vec2::new(0.0, -2.5)));
    world.insert(
        floor,
        Collider2D::Box {
            half_extents: Vec2::new(5.0, 0.5),
        },
    );
    platform(&mut world, 2.0);
    let me = character(&mut world, Vec2::new(0.0, -1.7));
    let mut velocity = Vec2::new(0.0, 7.0);
    for _ in 0..120 {
        velocity.y -= 10.0 * DT;
        velocity = move_and_slide(&mut world, me, velocity, DT)
            .expect("a character")
            .velocity;
    }
    let y = translation(&world, me).y;
    assert!((y - 0.26).abs() < 1e-3, "on the platform: {y}");
    let body = world.get::<CharacterBody2D>(me).expect("c");
    assert!(body.is_on_floor());
}

#[test]
fn a_character_rides_a_platform_that_moves_and_turns() {
    let mut world = World::new();
    let lift = world.spawn();
    world.insert(lift, Transform2D::default());
    world.insert(
        lift,
        Collider2D::Box {
            half_extents: Vec2::new(2.0, 0.1),
        },
    );
    let me = character(&mut world, Vec2::new(1.0, 0.5));
    let mut velocity = Vec2::ZERO;
    let fall = |world: &mut World, velocity: &mut Vec2| {
        velocity.y -= 10.0 * DT;
        *velocity = move_and_slide(world, me, *velocity, DT)
            .expect("c")
            .velocity;
    };
    for _ in 0..60 {
        fall(&mut world, &mut velocity);
    }
    let standing = translation(&world, me);
    let gap = standing.y - 0.1;

    // Across and up: carried by the same, and still on it.
    for _ in 0..60 {
        world
            .get_mut::<Transform2D>(lift)
            .expect("lift")
            .translation += Vec2::new(0.02, 0.01);
        fall(&mut world, &mut velocity);
    }
    let carried = translation(&world, me) - standing;
    assert!((carried - Vec2::new(1.2, 0.6)).length() < 0.02, "{carried}");
    let body = world.get::<CharacterBody2D>(me).expect("c");
    assert!(body.is_on_floor());
    assert!((body.contact.platform_velocity - Vec2::new(1.2, 0.6)).length() < 0.05);

    // A small turn carries it round the platform's centre.
    let before = translation(&world, me) - translation(&world, lift);
    world.get_mut::<Transform2D>(lift).expect("lift").rotation += 0.05;
    fall(&mut world, &mut velocity);
    let after = translation(&world, me) - translation(&world, lift);
    let expected = Vec2::from_angle(0.05).rotate(before);
    assert!(
        (after - expected).length() < gap + 0.02,
        "{after} vs {expected}"
    );

    // A platform that is gone carries nothing.
    world.despawn(lift);
    let at = translation(&world, me);
    move_and_slide(&mut world, me, Vec2::ZERO, DT).expect("c");
    assert_eq!(translation(&world, me), at);
}
