//! Pushing dynamic bodies: a crate shoved by a character walking into it,
//! with the physics step moving it between moves, as a game's tick does.

use voltra_physics::PhysicsWorld;
use voltra_scene::RigidBody;

use super::*;

const STEP_GRAVITY: Vec2 = Vec2::new(0.0, GRAVITY);

/// A dynamic box of `mass`, `half` either way of `at`.
fn crate_box(world: &mut World, at: Vec2, half: Vec2, mass: f32) -> Entity {
    let entity = block(world, at, half, 0.0);
    world.insert(entity, RigidBody::new_dynamic(mass));
    entity
}

fn pushing(world: &mut World, me: Entity, force: f32) {
    world
        .get_mut::<CharacterBody2D>(me)
        .expect("a character")
        .push_force = force;
}

/// `ticks` of a game walking `me` at `walk` and falling, then stepping the
/// world: answers the velocity the last move left.
fn play(world: &mut World, physics: &mut PhysicsWorld, me: Entity, walk: f32, ticks: u32) -> Vec2 {
    let mut velocity = Vec2::ZERO;
    for _ in 0..ticks {
        velocity = run(world, me, velocity, Some(walk), 1);
        physics.step_once(world, STEP_GRAVITY);
    }
    velocity
}

/// A floor, a character at the origin standing on it and a crate of `mass`
/// a little to its right, both settled.
fn crate_ahead(mass: f32, force: f32) -> (World, PhysicsWorld, Entity, Entity) {
    let mut world = World::new();
    floor(&mut world);
    let me = character(&mut world, Vec2::new(0.0, 0.3));
    let crate_ = crate_box(&mut world, Vec2::new(1.0, 0.26), Vec2::splat(0.25), mass);
    pushing(&mut world, me, force);
    let mut physics = PhysicsWorld::new();
    play(&mut world, &mut physics, me, 0.0, 60);
    (world, physics, me, crate_)
}

#[test]
fn walking_into_a_crate_pushes_it_and_keeps_walking() {
    let (mut world, mut physics, me, crate_) = crate_ahead(1.0, 40.0);
    let start = at(&world, crate_);
    let velocity = play(&mut world, &mut physics, me, 2.0, 90);
    let moved = at(&world, crate_) - start;
    assert!(moved.x > 1.5, "pushed along: {moved}");
    assert!(moved.y.abs() < 0.02, "along the floor: {moved}");
    // Never faster than the character walks into it.
    let speed = world.get::<RigidBody>(crate_).expect("a body").velocity.x;
    assert!(speed <= 2.0 + 1e-3, "{speed}");
    // A light crate hardly slows the character: it walks on at the speed
    // the crate made, which is its own less one step of the floor's
    // friction.
    assert!(velocity.x > 1.85 && velocity.x <= 2.0, "{velocity}");
    // And it never walks into the crate.
    let gap = at(&world, crate_).x - 0.25 - (at(&world, me).x + 0.25);
    assert!(gap > -1e-3, "{gap}");
}

#[test]
fn a_heavier_crate_moves_less_and_slows_the_character() {
    let walked = |mass: f32| {
        let (mut world, mut physics, me, crate_) = crate_ahead(mass, 40.0);
        let start = at(&world, crate_).x;
        let velocity = play(&mut world, &mut physics, me, 2.0, 60);
        (at(&world, crate_).x - start, velocity.x)
    };
    let (light, light_speed) = walked(1.0);
    let (heavy, heavy_speed) = walked(5.0);
    assert!(heavy > 0.05, "the heavy one still moves: {heavy}");
    assert!(heavy < light * 0.8, "{heavy} against {light}");
    assert!(
        heavy_speed < light_speed,
        "{heavy_speed} against {light_speed}"
    );
}

#[test]
fn a_push_force_of_zero_pushes_nothing() {
    let (mut world, mut physics, me, crate_) = crate_ahead(1.0, 0.0);
    let start = at(&world, crate_);
    let velocity = play(&mut world, &mut physics, me, 2.0, 60);
    assert!(
        (at(&world, crate_) - start).length() < 1e-3,
        "{}",
        at(&world, crate_)
    );
    assert!(body(&world, me).is_on_wall());
    assert!(velocity.x.abs() < 1e-4, "a wall like any other: {velocity}");
}

#[test]
fn a_crate_against_a_wall_is_not_pushed_through_it() {
    let (mut world, mut physics, me, crate_) = crate_ahead(1.0, 200.0);
    // The wall's face at x = 1.5, just right of the crate.
    block(&mut world, Vec2::new(2.0, 1.0), Vec2::new(0.5, 1.0), 0.0);
    for _ in 0..120 {
        play(&mut world, &mut physics, me, 4.0, 1);
        let crate_at = at(&world, crate_).x;
        assert!(crate_at + 0.25 <= 1.5 + 0.02, "into the wall: {crate_at}");
        let gap = crate_at - 0.25 - (at(&world, me).x + 0.25);
        assert!(gap > -1e-3, "into the crate: {gap}");
    }
    // Pinned, the crate takes all of the walk, as a wall does: no speed
    // kept for a walk animation to play against.
    let velocity = play(&mut world, &mut physics, me, 4.0, 1);
    assert!(velocity.x.abs() < 0.05, "{velocity}");
    assert!(body(&world, me).is_on_wall());
}

#[test]
fn a_crate_stood_on_goes_to_sleep() {
    let (mut world, mut physics, me, crate_) = crate_ahead(1.0, 40.0);
    world.get_mut::<Transform>(me).expect("placed").translation = Vec2::new(1.0, 0.9);
    play(&mut world, &mut physics, me, 0.0, 180);
    assert_eq!(body(&world, me).contact.floor, Some(crate_));
    assert!(physics.is_asleep(crate_), "pressed only by landing on it");
}

#[test]
fn a_crate_pushed_low_turns_and_one_pushed_square_does_not() {
    // No floor and no step: the impulse alone, on a tall crate floating
    // right of the character, met low and met across its middle.
    for (height, turns) in [(1.0f32, true), (0.25, false)] {
        let mut world = World::new();
        let me = character(&mut world, Vec2::new(0.0, 0.25));
        let crate_ = crate_box(
            &mut world,
            Vec2::new(0.5, height),
            Vec2::new(0.24, 1.0),
            1.0,
        );
        pushing(&mut world, me, 40.0);
        move_and_slide(&mut world, me, Vec2::new(2.0, 0.0), DT).expect("a character");
        let body = world.get::<RigidBody>(crate_).expect("a body");
        assert!(body.velocity.x > 0.1, "pushed: {}", body.velocity);
        assert_eq!(
            body.angular_velocity.abs() > 0.1,
            turns,
            "at {height}: {}",
            body.angular_velocity
        );
    }
}

#[test]
fn a_crate_despawned_mid_push_leaves_it_walking_on() {
    let (mut world, mut physics, me, crate_) = crate_ahead(1.0, 40.0);
    play(&mut world, &mut physics, me, 2.0, 30);
    world.despawn(crate_);
    let before = at(&world, me).x;
    let velocity = play(&mut world, &mut physics, me, 2.0, 30);
    assert!((at(&world, me).x - before - 1.0).abs() < 0.02);
    assert!((velocity.x - 2.0).abs() < 1e-4, "{velocity}");

    // Standing on one that goes is the same: it falls, and lands.
    let (mut world, mut physics, me, crate_) = crate_ahead(1.0, 40.0);
    world.get_mut::<Transform>(me).expect("placed").translation = Vec2::new(1.0, 1.0);
    play(&mut world, &mut physics, me, 0.0, 30);
    assert_eq!(body(&world, me).contact.floor, Some(crate_));
    world.despawn(crate_);
    play(&mut world, &mut physics, me, 0.0, 60);
    assert!(body(&world, me).is_on_floor());
    assert!(at(&world, me).y < 0.3, "{}", at(&world, me));
}
