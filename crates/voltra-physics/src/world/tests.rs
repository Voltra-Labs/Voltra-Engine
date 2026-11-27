//! The tests of `world`.

use super::*;
use crate::events::Touch;
use voltra_ecs::Entity;
use voltra_scene::{Collider, RigidBody, Sensor, Transform};

const G: Vec2 = Vec2::new(0.0, -10.0);

fn floor_and_ball() -> (World, Entity) {
    let mut world = World::new();

    let floor = world.spawn();
    world.insert(floor, Transform::from_translation(Vec2::new(0.0, -2.0)));
    world.insert(
        floor,
        Collider::Box {
            half_extents: Vec2::new(10.0, 0.5),
        },
    );
    world.insert(floor, RigidBody::new_static());

    let ball = world.spawn();
    world.insert(ball, Transform::default());
    world.insert(ball, Collider::Circle { radius: 0.5 });
    world.insert(ball, RigidBody::new_dynamic(1.0));

    (world, ball)
}

fn translation(world: &World, entity: Entity) -> Vec2 {
    world
        .get::<Transform>(entity)
        .expect("the transform is there")
        .translation
}

fn height(world: &World, entity: Entity) -> f32 {
    world
        .get::<Transform>(entity)
        .expect("the transform is there")
        .translation
        .y
}

#[test]
fn a_frame_shorter_than_the_step_simulates_nothing_yet() {
    let mut physics = PhysicsWorld::new();
    let (mut world, ball) = floor_and_ball();

    assert!(physics.advance(&mut world, G, 1.0 / 600.0).is_empty());
    assert_eq!(height(&world, ball), 0.0);
}

#[test]
fn a_long_frame_is_capped_rather_than_spiralling() {
    // The clock's eight-step cap, exercised through the solver: a ten
    // second frame must not run six hundred steps.
    let mut physics = PhysicsWorld::new();
    let (mut world, ball) = floor_and_ball();

    physics.advance(&mut world, G, 10.0);

    let y = height(&world, ball);
    assert!(y > -1.5, "at most eight steps of falling, got {y}");
}

#[test]
fn contacts_survive_a_frame_that_owed_no_step() {
    let mut physics = PhysicsWorld::new();
    let (mut world, _) = floor_and_ball();
    for _ in 0..300 {
        physics.advance(&mut world, G, 1.0 / 60.0);
    }
    assert!(!physics.contacts().is_empty(), "it should be resting");

    physics.advance(&mut world, G, 0.0);

    assert!(
        !physics.contacts().is_empty(),
        "a frame with no step must not blank them"
    );
}

#[test]
fn a_body_dropped_through_the_world_comes_to_rest() {
    let mut physics = PhysicsWorld::new();
    let (mut world, ball) = floor_and_ball();

    for _ in 0..300 {
        physics.advance(&mut world, G, 1.0 / 60.0);
    }

    assert!((height(&world, ball) + 1.0).abs() < 0.05);
    assert!(physics.cached_pairs() > 0, "the resting contact stays warm");
}

#[test]
fn resetting_forgets_the_accumulated_impulses() {
    let mut physics = PhysicsWorld::new();
    let (mut world, _) = floor_and_ball();
    for _ in 0..300 {
        physics.advance(&mut world, G, 1.0 / 60.0);
    }
    assert!(physics.cached_pairs() > 0);

    physics.reset();

    assert_eq!(physics.cached_pairs(), 0);
    assert!(physics.contacts().is_empty());
}

#[test]
fn step_once_advances_exactly_one_step() {
    // Same displacement as a frame that owed exactly one step, which is
    // what "one fixed step" has to mean for the editor's Step button.
    let mut once = PhysicsWorld::new();
    let (mut world_a, ball_a) = floor_and_ball();
    once.step_once(&mut world_a, G);

    let mut advanced = PhysicsWorld::new();
    let (mut world_b, ball_b) = floor_and_ball();
    advanced.advance(&mut world_b, G, 1.0 / 60.0);

    assert!(height(&world_a, ball_a) < 0.0, "it must have fallen");
    assert_eq!(height(&world_a, ball_a), height(&world_b, ball_b));
}

#[test]
fn step_once_ignores_the_banked_time() {
    // The accumulator is the clock's business. A Step press must run one
    // step and neither spend nor add to the debt a frame is carrying.
    let mut physics = PhysicsWorld::new();
    let (mut world, ball) = floor_and_ball();
    physics.advance(&mut world, G, 1.0 / 600.0);
    assert_eq!(height(&world, ball), 0.0, "a tenth of a step runs nothing");

    physics.step_once(&mut world, G);
    let after_one = height(&world, ball);

    physics.advance(&mut world, G, 0.0);
    assert_eq!(
        height(&world, ball),
        after_one,
        "the step must not have consumed or topped up the bank"
    );
}

#[test]
fn resetting_also_clears_the_clock() {
    let mut physics = PhysicsWorld::new();
    let (mut world, ball) = floor_and_ball();
    physics.advance(&mut world, G, (1.0 / 60.0) * 0.9);
    assert_eq!(height(&world, ball), 0.0, "0.9 of a step owes nothing yet");

    physics.reset();

    physics.advance(&mut world, G, (1.0 / 60.0) * 0.2);
    assert_eq!(height(&world, ball), 0.0, "the banked 0.9 must be gone");
}

#[test]
fn the_owed_steps_are_consumed_by_asking() {
    // A caller that steps them itself must not be able to ask twice and
    // run the same debt twice.
    let mut physics = PhysicsWorld::new();

    assert_eq!(physics.owed_steps(2.5 / 60.0), 2);
    assert_eq!(
        physics.owed_steps(0.0),
        0,
        "the debt was already handed out"
    );
    assert_eq!(
        physics.owed_steps(0.6 / 60.0),
        1,
        "and the half step left over is still banked"
    );
}

#[test]
fn asking_for_the_owed_steps_is_what_advance_does() {
    let mut counted = PhysicsWorld::new();
    let (mut counted_world, counted_ball) = floor_and_ball();
    for _ in 0..counted.owed_steps(3.0 / 60.0) {
        counted.step_once(&mut counted_world, G);
    }

    let mut advanced = PhysicsWorld::new();
    let (mut advanced_world, advanced_ball) = floor_and_ball();
    advanced.advance(&mut advanced_world, G, 3.0 / 60.0);

    assert_eq!(
        height(&counted_world, counted_ball),
        height(&advanced_world, advanced_ball)
    );
}

/// A static box at the origin and a dynamic ball sitting inside it.
fn overlapping(sensor: bool) -> (PhysicsWorld, World, Entity, Entity) {
    let mut world = World::new();

    let trigger = world.spawn();
    world.insert(trigger, Transform::default());
    world.insert(
        trigger,
        Collider::Box {
            half_extents: Vec2::splat(1.0),
        },
    );
    if sensor {
        world.insert(trigger, Sensor);
    }

    let ball = world.spawn();
    world.insert(ball, Transform::from_translation(Vec2::new(0.25, 0.0)));
    world.insert(ball, Collider::Circle { radius: 0.5 });
    world.insert(ball, RigidBody::new_dynamic(1.0));

    (PhysicsWorld::new(), world, trigger, ball)
}

#[test]
fn a_sensor_is_detected_and_never_solved() {
    let (mut physics, mut world, trigger, ball) = overlapping(true);
    let before = translation(&world, ball);

    for _ in 0..10 {
        physics.step_once(&mut world, Vec2::ZERO);
    }

    assert!(
        physics.contacts().is_empty(),
        "a sensor overlap is not a contact"
    );
    assert_eq!(physics.cached_pairs(), 0, "and never warm starts");
    assert_eq!(translation(&world, ball), before, "nor pushes anything out");
    assert!(physics.touching().contains(trigger, ball));
}

#[test]
fn the_same_overlap_without_the_sensor_is_solved() {
    // The other half of the test above: what changes is the mark, not the
    // scene.
    let (mut physics, mut world, _, ball) = overlapping(false);
    let before = translation(&world, ball);

    for _ in 0..10 {
        physics.step_once(&mut world, Vec2::ZERO);
    }

    assert_eq!(physics.contacts().len(), 1);
    assert!(translation(&world, ball).x > before.x, "it is pushed out");
}

#[test]
fn a_pair_begins_once_and_then_says_nothing() {
    let (mut physics, mut world, trigger, ball) = overlapping(true);

    physics.step_once(&mut world, Vec2::ZERO);
    assert_eq!(
        physics.events(),
        [CollisionEvent {
            a: trigger,
            b: ball,
            touch: Touch::Began,
            sensor: true,
        }]
    );

    physics.step_once(&mut world, Vec2::ZERO);
    assert!(physics.events().is_empty(), "it is still the same overlap");
}

#[test]
fn walking_out_of_a_sensor_ends_the_pair() {
    let (mut physics, mut world, trigger, ball) = overlapping(true);
    physics.step_once(&mut world, Vec2::ZERO);

    world
        .get_mut::<Transform>(ball)
        .expect("the transform is there")
        .translation = Vec2::new(50.0, 0.0);
    physics.step_once(&mut world, Vec2::ZERO);

    assert_eq!(
        physics.events(),
        [CollisionEvent {
            a: trigger,
            b: ball,
            touch: Touch::Ended,
            sensor: true,
        }]
    );
}

#[test]
fn despawning_one_side_ends_the_pair_rather_than_losing_it() {
    // What a pickup does to itself. Silence here would leave a door that
    // opened on Began with nothing to close it.
    let (mut physics, mut world, trigger, ball) = overlapping(true);
    physics.step_once(&mut world, Vec2::ZERO);

    world.despawn(trigger);
    physics.step_once(&mut world, Vec2::ZERO);

    assert_eq!(physics.events().len(), 1);
    assert_eq!(physics.events()[0].touch, Touch::Ended);
    assert_eq!(physics.events()[0].other(ball), Some(trigger));
}

#[test]
fn resetting_ends_nothing() {
    let (mut physics, mut world, _, _) = overlapping(true);
    physics.step_once(&mut world, Vec2::ZERO);
    assert_eq!(physics.touching().len(), 1);

    physics.reset();

    assert!(physics.events().is_empty());
    assert!(physics.touching().is_empty());
    physics.step_once(&mut world, Vec2::ZERO);
    assert_eq!(
        physics.events().len(),
        1,
        "the pair begins again, and never ended"
    );
    assert_eq!(physics.events()[0].touch, Touch::Began);
}

#[test]
fn a_frame_that_owed_no_step_leaves_the_events_alone() {
    // Same rule the contacts have: a fast frame reports the frame rate if
    // it blanks what the last step found.
    let (mut physics, mut world, _, _) = overlapping(true);
    physics.advance(&mut world, Vec2::ZERO, 1.0 / 60.0);
    assert_eq!(physics.events().len(), 1);

    physics.advance(&mut world, Vec2::ZERO, 0.0);

    assert_eq!(physics.events().len(), 1);
}

#[test]
fn a_world_stepping_at_zero_falls_back_to_the_default_rate() {
    let mut physics = PhysicsWorld::with_step(0.0);
    let (mut world, ball) = floor_and_ball();

    physics.advance(&mut world, G, 1.0 / 60.0);

    assert!(height(&world, ball) < 0.0, "it should have run one step");
}
