//! Joints measured by where the scene ends up: a pendulum keeps its length,
//! a rope goes slack and taut, a weld holds, a limit stops, a motor turns,
//! joined bodies pass through each other, and a jointed island sleeps as one.

mod common;

use common::*;
use voltra_ecs::{Entity, World};
use voltra_physics::{ImpulseCache, PhysicsWorld};
use voltra_render::glam::Vec2;
use voltra_scene::{
    AngleLimit, Collider, Joint2D, JointKind, Motor, RigidBody, SceneId, Spring, Transform,
};

/// A dynamic ball of radius 0.2 at `at`.
fn ball(world: &mut World, at: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Transform::from_translation(at));
    world.insert(entity, Collider::Circle { radius: 0.2 });
    world.insert(entity, RigidBody::new_dynamic(1.0));
    entity
}

/// `kind` on `entity`, held to the world at `pivot`.
fn to_world(world: &mut World, entity: Entity, pivot: Vec2, kind: JointKind) {
    world.insert(
        entity,
        Joint2D {
            anchor_b: pivot,
            kind,
            ..Joint2D::default()
        },
    );
}

fn id(world: &World, entity: Entity) -> SceneId {
    *world.get::<SceneId>(entity).expect("an id")
}

#[test]
fn a_rod_pendulum_swings_at_its_length() {
    let mut world = World::new();
    let bob = ball(&mut world, Vec2::new(1.0, 0.0));
    to_world(&mut world, bob, Vec2::ZERO, JointKind::rod(1.0));
    let mut cache = ImpulseCache::default();
    let mut lowest = f32::INFINITY;
    for _ in 0..240 {
        run(&mut world, &mut cache, G, 1);
        let at = translation(&world, bob);
        lowest = lowest.min(at.y);
        let length = at.length();
        assert!((length - 1.0).abs() < 0.02, "the rod holds: {length}");
    }
    assert!(lowest < -0.9, "it swung through the bottom: {lowest}");
}

#[test]
fn a_revolute_pendulum_keeps_its_radius() {
    let mut world = World::new();
    let bob = ball(&mut world, Vec2::new(1.0, 0.0));
    // Pinned one unit to its left, to the world.
    world.insert(
        bob,
        Joint2D {
            anchor_a: Vec2::new(-1.0, 0.0),
            anchor_b: Vec2::ZERO,
            ..Joint2D::default()
        },
    );
    let mut cache = ImpulseCache::default();
    let mut lowest = f32::INFINITY;
    for _ in 0..240 {
        run(&mut world, &mut cache, G, 1);
        let at = translation(&world, bob);
        lowest = lowest.min(at.y);
        let radius = at.length();
        assert!(
            (radius - 1.0).abs() < 0.02,
            "the pin holds: radius {radius}"
        );
    }
    assert!(lowest < -0.9, "it swung through the bottom: {lowest}");
}

#[test]
fn a_rod_keeps_two_falling_bodies_apart() {
    let mut world = World::new();
    let a = ball(&mut world, Vec2::new(0.0, 0.0));
    let b = ball(&mut world, Vec2::new(1.5, 0.5));
    let b_id = id(&world, b);
    world.insert(
        a,
        Joint2D {
            b: Some(b_id),
            kind: JointKind::rod(1.0),
            ..Joint2D::default()
        },
    );
    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 120);
    let gap = (translation(&world, b) - translation(&world, a)).length();
    assert!((gap - 1.0).abs() < 0.02, "pulled to its length: {gap}");
}

#[test]
fn a_rope_is_slack_until_it_is_taut() {
    let mut world = World::new();
    let bob = ball(&mut world, Vec2::new(0.0, -0.3));
    to_world(&mut world, bob, Vec2::ZERO, JointKind::rope(1.0));
    let mut cache = ImpulseCache::default();
    // Falling freely while slack: after a few frames it is where a free body
    // would be.
    run(&mut world, &mut cache, G, 6);
    let free_fall = -0.3 - 0.5 * 10.0 * (6.0 * DT) * (6.0 * DT);
    assert!(
        (translation(&world, bob).y - free_fall).abs() < 0.02,
        "slack: {} against {free_fall}",
        translation(&world, bob).y
    );
    run(&mut world, &mut cache, G, 120);
    let hang = translation(&world, bob).length();
    assert!((hang - 1.0).abs() < 0.02, "taut at its length: {hang}");
}

#[test]
fn a_spring_stretches_under_its_load_and_settles() {
    let mut world = World::new();
    let bob = ball(&mut world, Vec2::new(0.0, -1.0));
    to_world(
        &mut world,
        bob,
        Vec2::ZERO,
        JointKind::Distance {
            length: 1.0,
            min: 1.0,
            max: 1.0,
            spring: Some(Spring {
                hertz: 2.0,
                damping_ratio: 0.7,
            }),
        },
    );
    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 600);
    let hang = translation(&world, bob).length();
    // k = m(2π f)² ≈ 158, so it stretches by g/k ≈ 0.063.
    assert!((hang - 1.063).abs() < 0.02, "stretched by its load: {hang}");
    assert!(velocity(&world, bob).length() < 0.01, "and settled");
}

#[test]
fn a_weld_to_the_world_holds_a_body_in_place() {
    let mut world = World::new();
    let plank = spawn_box(
        &mut world,
        Vec2::new(2.0, 0.0),
        Vec2::new(1.0, 0.1),
        Some(1.0),
    );
    world.insert(plank, SceneId::new());
    to_world(
        &mut world,
        plank,
        Vec2::new(1.0, 0.0),
        JointKind::Weld {
            reference_angle: 0.0,
        },
    );
    // Held at its left end, at the world point where that end is.
    world.get_mut::<Joint2D>(plank).expect("joint").anchor_a = Vec2::new(-1.0, 0.0);
    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 240);
    assert!((translation(&world, plank) - Vec2::new(2.0, 0.0)).length() < 0.03);
    assert!(
        rotation(&world, plank).abs() < 0.03,
        "and it does not droop"
    );
}

#[test]
fn a_limit_stops_a_hinge_turning_past_it() {
    let mut world = World::new();
    let bob = ball(&mut world, Vec2::new(1.0, 0.0));
    world.insert(
        bob,
        Joint2D {
            anchor_a: Vec2::new(-1.0, 0.0),
            kind: JointKind::Revolute {
                reference_angle: 0.0,
                limit: Some(AngleLimit {
                    lower: -0.5,
                    upper: 0.5,
                }),
                motor: None,
            },
            ..Joint2D::default()
        },
    );
    let mut cache = ImpulseCache::default();
    for _ in 0..240 {
        run(&mut world, &mut cache, G, 1);
        let turned = rotation(&world, bob);
        assert!(turned > -0.55, "held at its lower limit: {turned}");
    }
    let rest = rotation(&world, bob);
    assert!((rest + 0.5).abs() < 0.05, "resting on it: {rest}");
}

#[test]
fn a_motor_turns_a_wheel_at_its_speed() {
    let mut world = World::new();
    let wheel = ball(&mut world, Vec2::ZERO);
    to_world(
        &mut world,
        wheel,
        Vec2::ZERO,
        JointKind::Revolute {
            reference_angle: 0.0,
            limit: None,
            motor: Some(Motor {
                speed: 3.0,
                max_torque: 100.0,
            }),
        },
    );
    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, Vec2::ZERO, 60);
    let speed = spin(&world, wheel);
    assert!((speed - 3.0).abs() < 0.05, "at its speed: {speed}");
    assert!(translation(&world, wheel).length() < 0.01, "on its axle");
}

#[test]
fn joined_bodies_pass_through_each_other_unless_asked_not_to() {
    let overlap = |collide: bool| {
        let mut world = World::new();
        let a = ball(&mut world, Vec2::ZERO);
        let b = ball(&mut world, Vec2::new(0.1, 0.0));
        let b_id = id(&world, b);
        world.insert(
            a,
            Joint2D {
                b: Some(b_id),
                collide_connected: collide,
                kind: JointKind::rope(5.0),
                ..Joint2D::default()
            },
        );
        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, Vec2::ZERO, 30);
        (translation(&world, b) - translation(&world, a)).length()
    };
    assert!(
        overlap(false) < 0.15,
        "left overlapping: {}",
        overlap(false)
    );
    assert!(overlap(true) > 0.35, "pushed apart: {}", overlap(true));
}

#[test]
fn a_hanging_pendulum_sleeps_and_a_motor_keeps_its_wheel_awake() {
    let mut physics = PhysicsWorld::new();
    let mut world = World::new();
    let bob = ball(&mut world, Vec2::new(0.0, -1.0));
    world.insert(
        bob,
        Joint2D {
            anchor_a: Vec2::new(0.0, 1.0),
            ..Joint2D::default()
        },
    );
    let wheel = ball(&mut world, Vec2::new(5.0, 0.0));
    to_world(
        &mut world,
        wheel,
        Vec2::new(5.0, 0.0),
        JointKind::Revolute {
            reference_angle: 0.0,
            limit: None,
            motor: Some(Motor {
                speed: 1.0,
                max_torque: 10.0,
            }),
        },
    );
    for _ in 0..300 {
        physics.step_once(&mut world, G);
    }
    assert!(physics.is_asleep(bob), "hanging still, it sleeps");
    assert!(!physics.is_asleep(wheel), "driven, it does not");
}
