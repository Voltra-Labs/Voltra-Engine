//! Bodies at rest go to sleep by island, stay put, and wake when disturbed.

use voltra_ecs::{Entity, World};
use voltra_physics::{PhysicsWorld, Touch};
use voltra_render::glam::Vec2;
use voltra_scene::{BodyType, Collider, RigidBody, Transform};

const G: Vec2 = Vec2::new(0.0, -10.0);
const STEP: f32 = 1.0 / 60.0;

fn floor(world: &mut World, at: Vec2, half: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform::from_translation(at));
    world.insert(entity, Collider::Box { half_extents: half });
    entity
}

fn crate_at(world: &mut World, at: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform::from_translation(at));
    world.insert(entity, RigidBody::new_dynamic(1.0));
    world.insert(
        entity,
        Collider::Box {
            half_extents: Vec2::splat(0.5),
        },
    );
    entity
}

fn run(physics: &mut PhysicsWorld, world: &mut World, seconds: f32) {
    for _ in 0..(seconds / STEP).round() as usize {
        physics.step_once(world, G);
    }
}

fn at(world: &World, entity: Entity) -> Vec2 {
    world
        .get::<Transform>(entity)
        .expect("a transform")
        .translation
}

/// A floor and one crate resting on it, asleep.
fn asleep_on_a_floor() -> (PhysicsWorld, World, Entity, Entity) {
    let mut world = World::new();
    let ground = floor(&mut world, Vec2::new(0.0, -0.5), Vec2::new(10.0, 0.5));
    let body = crate_at(&mut world, Vec2::new(0.0, 0.6));
    let mut physics = PhysicsWorld::with_step(STEP);
    run(&mut physics, &mut world, 3.0);
    assert!(
        physics.is_asleep(body),
        "settled and asleep after three seconds"
    );
    (physics, world, ground, body)
}

#[test]
fn a_crate_that_comes_to_rest_falls_asleep() {
    let (physics, _, _, _) = asleep_on_a_floor();
    assert_eq!(physics.asleep_count(), 1);
}

#[test]
fn a_sleeping_crate_stays_exactly_where_it_slept() {
    let (mut physics, mut world, _, body) = asleep_on_a_floor();
    let before = at(&world, body);
    run(&mut physics, &mut world, 5.0);
    assert_eq!(at(&world, body), before, "no gravity creep, no jitter");
}

#[test]
fn a_sleeping_crate_is_still_touching_its_floor() {
    let (mut physics, mut world, ground, body) = asleep_on_a_floor();
    physics.step_once(&mut world, G);
    assert!(physics.touching().contains(ground, body));
    assert!(
        physics
            .contacts()
            .iter()
            .any(|c| (c.a, c.b) == (ground, body)),
        "a ground check by contacts still sees it standing"
    );
    assert!(
        physics.events().iter().all(|e| e.touch != Touch::Ended),
        "going to sleep is not leaving the floor"
    );
}

#[test]
fn a_velocity_written_by_the_game_wakes_it() {
    let (mut physics, mut world, _, body) = asleep_on_a_floor();
    let resting = at(&world, body);
    world.get_mut::<RigidBody>(body).expect("a body").velocity = Vec2::new(0.0, 5.0);
    physics.step_once(&mut world, G);
    assert!(!physics.is_asleep(body));
    assert!(at(&world, body).y > resting.y, "and it moved this step");
}

#[test]
fn moving_its_transform_from_outside_wakes_it() {
    let (mut physics, mut world, _, body) = asleep_on_a_floor();
    world
        .get_mut::<Transform>(body)
        .expect("a transform")
        .translation
        .y = 3.0;
    run(&mut physics, &mut world, 0.2);
    assert!(!physics.is_asleep(body));
    assert!(at(&world, body).y < 3.0, "falling again");
}

#[test]
fn losing_its_support_wakes_it() {
    let (mut physics, mut world, ground, body) = asleep_on_a_floor();
    world.despawn(ground);
    run(&mut physics, &mut world, 0.5);
    assert!(!physics.is_asleep(body));
    assert!(at(&world, body).y < 0.0, "it fell");
}

#[test]
fn something_awake_landing_on_a_sleeping_stack_wakes_the_whole_stack() {
    let mut world = World::new();
    floor(&mut world, Vec2::new(0.0, -0.5), Vec2::new(10.0, 0.5));
    let bottom = crate_at(&mut world, Vec2::new(0.0, 0.5));
    let top = crate_at(&mut world, Vec2::new(0.0, 1.5));
    let mut physics = PhysicsWorld::with_step(STEP);
    run(&mut physics, &mut world, 3.0);
    assert!(physics.is_asleep(bottom) && physics.is_asleep(top));

    let falling = crate_at(&mut world, Vec2::new(0.0, 4.0));
    let mut woke = false;
    for _ in 0..120 {
        physics.step_once(&mut world, G);
        woke |= !physics.is_asleep(bottom);
    }
    assert!(woke, "the bottom crate woke through the one it was holding");
    assert!(world.get::<Transform>(falling).is_some());
}

#[test]
fn a_disturbance_wakes_only_its_own_island() {
    let mut world = World::new();
    floor(&mut world, Vec2::new(0.0, -0.5), Vec2::new(20.0, 0.5));
    let left = crate_at(&mut world, Vec2::new(-5.0, 0.5));
    let right = crate_at(&mut world, Vec2::new(5.0, 0.5));
    let mut physics = PhysicsWorld::with_step(STEP);
    run(&mut physics, &mut world, 3.0);

    world.get_mut::<RigidBody>(left).expect("a body").velocity = Vec2::new(1.0, 0.0);
    physics.step_once(&mut world, G);

    assert!(!physics.is_asleep(left));
    assert!(
        physics.is_asleep(right),
        "sharing a floor does not join islands"
    );
}

#[test]
fn a_body_that_may_not_sleep_never_does() {
    let mut world = World::new();
    floor(&mut world, Vec2::new(0.0, -0.5), Vec2::new(10.0, 0.5));
    let body = crate_at(&mut world, Vec2::new(0.0, 0.6));
    world.get_mut::<RigidBody>(body).expect("a body").can_sleep = false;
    let mut physics = PhysicsWorld::with_step(STEP);
    run(&mut physics, &mut world, 3.0);
    assert!(!physics.is_asleep(body));
}

#[test]
fn sleeping_switched_off_simulates_everything() {
    let mut world = World::new();
    floor(&mut world, Vec2::new(0.0, -0.5), Vec2::new(10.0, 0.5));
    let body = crate_at(&mut world, Vec2::new(0.0, 0.6));
    let mut physics = PhysicsWorld::with_step(STEP);
    physics.params_mut().sleep.enabled = false;
    run(&mut physics, &mut world, 3.0);
    assert!(!physics.is_asleep(body));
    assert_eq!(physics.asleep_count(), 0);
}

#[test]
fn a_moving_kinematic_platform_keeps_its_rider_awake() {
    let mut world = World::new();
    let platform = floor(&mut world, Vec2::new(0.0, -0.5), Vec2::new(10.0, 0.5));
    world.insert(
        platform,
        RigidBody {
            body_type: BodyType::Kinematic,
            velocity: Vec2::new(0.5, 0.0),
            ..RigidBody::new_static()
        },
    );
    let rider = crate_at(&mut world, Vec2::new(0.0, 0.6));
    let mut physics = PhysicsWorld::with_step(STEP);
    run(&mut physics, &mut world, 3.0);
    assert!(!physics.is_asleep(rider));
}

#[test]
fn a_platform_that_starts_moving_wakes_a_sleeping_rider() {
    let mut world = World::new();
    let platform = floor(&mut world, Vec2::new(0.0, -0.5), Vec2::new(10.0, 0.5));
    world.insert(
        platform,
        RigidBody {
            body_type: BodyType::Kinematic,
            ..RigidBody::new_static()
        },
    );
    let rider = crate_at(&mut world, Vec2::new(0.0, 0.6));
    let mut physics = PhysicsWorld::with_step(STEP);
    run(&mut physics, &mut world, 3.0);
    assert!(physics.is_asleep(rider));

    world
        .get_mut::<RigidBody>(platform)
        .expect("a body")
        .velocity = Vec2::new(0.0, 1.0);
    physics.step_once(&mut world, G);
    assert!(!physics.is_asleep(rider));
}

#[test]
fn reset_wakes_everything() {
    let (mut physics, _, _, body) = asleep_on_a_floor();
    physics.reset();
    assert!(!physics.is_asleep(body));
}

#[test]
fn wake_from_code_wakes_it() {
    let (mut physics, _, _, body) = asleep_on_a_floor();
    physics.wake(body);
    assert!(!physics.is_asleep(body));
}
