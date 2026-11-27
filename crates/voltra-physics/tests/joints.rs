//! Joints measured by where the scene ends up: a pendulum keeps its length,
//! a rope goes slack and taut, a weld holds, a limit stops, a motor turns,
//! joined bodies pass through each other, and a jointed island sleeps as one;
//! a wheel's suspension settles, a slider stops at its limits and is driven,
//! a mouse joint pulls, a revolute spring returns, and a joint loaded past
//! what it holds breaks.

mod common;

use common::*;
use voltra_ecs::{Entity, World};
use voltra_physics::{ImpulseCache, PhysicsWorld};
use voltra_render::glam::Vec2;
use voltra_scene::{
    AngleLimit, Collider, Joint2D, JointKind, LinearMotor, Motor, RestSpring, RigidBody, SceneId,
    Spring, Transform, TranslationLimit,
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
                spring: None,
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
            spring: None,
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
            spring: None,
        },
    );
    for _ in 0..300 {
        physics.step_once(&mut world, G);
    }
    assert!(physics.is_asleep(bob), "hanging still, it sleeps");
    assert!(!physics.is_asleep(wheel), "driven, it does not");
}

/// A box of `half` extents and `mass` at `at`, with an id.
fn plank(world: &mut World, at: Vec2, half: Vec2, mass: f32) -> Entity {
    let entity = spawn_box(world, at, half, Some(mass));
    world.insert(entity, SceneId::new());
    entity
}

/// How far `wheel` sits along `chassis`'s up from where its anchor holds it.
fn suspension(world: &World, wheel: Entity, chassis: Entity) -> f32 {
    let joint = world.get::<Joint2D>(wheel).expect("the wheel's joint");
    let body = world.get::<Transform>(chassis).expect("the chassis");
    let anchor = body.matrix().transform_point2(joint.anchor_b);
    let up = Vec2::from_angle(body.rotation).rotate(Vec2::Y);
    (translation(world, wheel) - anchor).dot(up)
}

#[test]
fn a_parked_car_on_a_slope_settles_on_its_suspension() {
    let slope = 0.15_f32;
    let mut world = World::new();
    let ground = spawn_turned_box(
        &mut world,
        Vec2::new(0.0, -1.0),
        Vec2::new(10.0, 0.5),
        None,
        slope,
    );
    let _ = ground;
    // A chassis over two wheels, the whole car turned to the slope.
    let turn = |v: Vec2| Vec2::from_angle(slope).rotate(v);
    let base = Vec2::new(0.0, -1.0) + turn(Vec2::new(0.0, 0.5 + 0.3));
    let chassis = plank(
        &mut world,
        base + turn(Vec2::new(0.0, 0.6)),
        Vec2::new(1.0, 0.2),
        2.0,
    );
    world
        .get_mut::<Transform>(chassis)
        .expect("chassis")
        .rotation = slope;
    let chassis_id = id(&world, chassis);
    let mut wheels = Vec::new();
    for x in [-0.7, 0.7] {
        let wheel = world.spawn();
        world.insert(wheel, SceneId::new());
        world.insert(
            wheel,
            Transform::from_translation(base + turn(Vec2::new(x, 0.0))),
        );
        world.insert(wheel, Collider::Circle { radius: 0.3 });
        world.insert(wheel, RigidBody::new_dynamic(0.5));
        world.insert(
            wheel,
            Joint2D {
                b: Some(chassis_id),
                anchor_b: Vec2::new(x, -0.6),
                kind: JointKind::Wheel {
                    axis: Vec2::Y,
                    spring: Some(RestSpring {
                        hertz: 4.0,
                        damping_ratio: 0.7,
                        rest: 0.0,
                    }),
                    limit: Some(TranslationLimit {
                        lower: -0.3,
                        upper: 0.3,
                    }),
                    // The parking brake: held at no speed.
                    motor: Some(Motor {
                        speed: 0.0,
                        max_torque: 50.0,
                    }),
                },
                ..Joint2D::default()
            },
        );
        wheels.push(wheel);
    }
    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 180);
    let before: Vec<f32> = wheels
        .iter()
        .map(|&w| suspension(&world, w, chassis))
        .collect();
    run(&mut world, &mut cache, G, 60);
    for (&wheel, before) in wheels.iter().zip(before) {
        let now = suspension(&world, wheel, chassis);
        assert!(
            now > 0.0 && now < 0.3,
            "the chassis sits down on its springs: {now}"
        );
        assert!(
            (now - before).abs() < 0.01,
            "and has settled: {before} then {now}"
        );
    }
    assert!(
        velocity(&world, chassis).length() < 0.05,
        "parked: {}",
        velocity(&world, chassis)
    );
    assert!(
        (rotation(&world, chassis) - slope).abs() < 0.05,
        "along the slope: {}",
        rotation(&world, chassis)
    );
}

#[test]
fn a_wheel_motor_drives_a_car_along() {
    let mut world = World::new();
    spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(50.0, 0.5), None);
    let chassis = plank(&mut world, Vec2::new(0.0, 0.4), Vec2::new(1.0, 0.2), 2.0);
    let chassis_id = id(&world, chassis);
    for x in [-0.7, 0.7] {
        let wheel = ball(&mut world, Vec2::new(x, -0.2));
        world.insert(wheel, Collider::Circle { radius: 0.3 });
        world.insert(
            wheel,
            Joint2D {
                b: Some(chassis_id),
                anchor_b: Vec2::new(x, -0.6),
                kind: JointKind::Wheel {
                    axis: Vec2::Y,
                    spring: Some(RestSpring::default()),
                    limit: None,
                    // Clockwise: rolling to the right.
                    motor: Some(Motor {
                        speed: -10.0,
                        max_torque: 20.0,
                    }),
                },
                ..Joint2D::default()
            },
        );
    }
    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 120);
    assert!(
        translation(&world, chassis).x > 2.0,
        "driven to the right: {}",
        translation(&world, chassis)
    );
}

#[test]
fn a_slider_is_driven_to_its_limit_and_stops_there() {
    let mut world = World::new();
    let slider = plank(&mut world, Vec2::ZERO, Vec2::splat(0.2), 1.0);
    to_world(
        &mut world,
        slider,
        Vec2::ZERO,
        JointKind::Prismatic {
            axis: Vec2::X,
            reference_angle: 0.0,
            limit: Some(TranslationLimit {
                lower: -1.0,
                upper: 1.0,
            }),
            motor: Some(LinearMotor {
                speed: 2.0,
                max_force: 100.0,
            }),
            spring: None,
        },
    );
    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 15);
    let early = velocity(&world, slider).x;
    assert!((early - 2.0).abs() < 0.05, "at the motor's speed: {early}");
    for _ in 0..120 {
        run(&mut world, &mut cache, G, 1);
        let at = translation(&world, slider);
        assert!(at.x < 1.02, "stopped at its upper limit: {at}");
        assert!(at.y.abs() < 0.01, "held on its line under gravity: {at}");
        assert!(rotation(&world, slider).abs() < 0.01, "and its angle");
    }
    assert!((translation(&world, slider).x - 1.0).abs() < 0.02);
    // Driven back the other way, to the lower one.
    if let Some(Joint2D {
        kind: JointKind::Prismatic { motor, .. },
        ..
    }) = world.get_mut::<Joint2D>(slider)
    {
        *motor = Some(LinearMotor {
            speed: -2.0,
            max_force: 100.0,
        });
    }
    run(&mut world, &mut cache, G, 120);
    let back = translation(&world, slider).x;
    assert!((back + 1.0).abs() < 0.02, "at its lower limit: {back}");
}

#[test]
fn a_sprung_slider_rests_where_its_load_balances_the_spring() {
    let mut world = World::new();
    let slider = plank(&mut world, Vec2::ZERO, Vec2::splat(0.2), 1.0);
    to_world(
        &mut world,
        slider,
        Vec2::ZERO,
        JointKind::Prismatic {
            axis: Vec2::Y,
            reference_angle: 0.0,
            limit: None,
            motor: None,
            spring: Some(RestSpring {
                hertz: 2.0,
                damping_ratio: 0.7,
                rest: 0.5,
            }),
        },
    );
    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 600);
    // k = m(2π f)² ≈ 158: g/k ≈ 0.063 under its rest.
    let y = translation(&world, slider).y;
    assert!((y - (0.5 - 0.063)).abs() < 0.02, "hangs on its spring: {y}");
}

#[test]
fn a_mouse_joint_pulls_a_body_to_its_target() {
    let mut world = World::new();
    let body = ball(&mut world, Vec2::ZERO);
    let target = Vec2::new(2.0, 1.0);
    to_world(&mut world, body, target, JointKind::mouse());
    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 120);
    let at = translation(&world, body);
    assert!((at - target).length() < 0.05, "pulled to {target}: {at}");
    assert!(velocity(&world, body).length() < 0.05, "and held there");

    // The pull is capped: five newtons cannot hold a kilogram up.
    let mut world = World::new();
    let heavy = ball(&mut world, Vec2::ZERO);
    to_world(
        &mut world,
        heavy,
        Vec2::ZERO,
        JointKind::Mouse {
            max_force: 5.0,
            hertz: 5.0,
            damping_ratio: 0.7,
        },
    );
    run(&mut world, &mut cache, G, 60);
    assert!(
        translation(&world, heavy).y < -1.0,
        "it sinks under its weight: {}",
        translation(&world, heavy)
    );
}

#[test]
fn a_revolute_spring_turns_a_bar_back_to_rest() {
    for rest in [0.0, 0.5] {
        let mut world = World::new();
        let bar = plank(&mut world, Vec2::ZERO, Vec2::new(1.0, 0.1), 1.0);
        world
            .get_mut::<RigidBody>(bar)
            .expect("body")
            .angular_velocity = 5.0;
        to_world(
            &mut world,
            bar,
            Vec2::ZERO,
            JointKind::Revolute {
                reference_angle: 0.0,
                limit: None,
                motor: None,
                spring: Some(RestSpring {
                    hertz: 2.0,
                    damping_ratio: 0.5,
                    rest,
                }),
            },
        );
        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, Vec2::ZERO, 10);
        assert!(rotation(&world, bar) > 0.1, "it turned away first");
        run(&mut world, &mut cache, Vec2::ZERO, 240);
        let angle = rotation(&world, bar);
        assert!((angle - rest).abs() < 0.02, "back at {rest}: {angle}");
        assert!(spin(&world, bar).abs() < 0.02, "and still");
    }
}

#[test]
fn a_joint_breaks_once_past_its_force_and_despawning_after_is_fine() {
    let mut physics = PhysicsWorld::new();
    let mut world = World::new();
    // Released level, a kilogram on a metre's rod pulls 3mg = 30 N at the
    // bottom of its swing; hanging still it pulls 10.
    let swinging = ball(&mut world, Vec2::new(1.0, 0.0));
    let hanging = ball(&mut world, Vec2::new(5.0, -1.0));
    for (bob, pivot) in [(swinging, Vec2::ZERO), (hanging, Vec2::new(5.0, 0.0))] {
        world.insert(
            bob,
            Joint2D {
                anchor_b: pivot,
                kind: JointKind::rod(1.0),
                break_force: Some(25.0),
                ..Joint2D::default()
            },
        );
    }
    let mut events = Vec::new();
    for _ in 0..120 {
        physics.step_once(&mut world, G);
        events.extend_from_slice(physics.broken_joints());
    }
    assert_eq!(events.len(), 1, "one break, reported once: {events:?}");
    let broke = events[0];
    assert_eq!(broke.entity, swinging);
    assert!(broke.joined(swinging) && broke.b.is_none());
    assert!(broke.force > 25.0, "past its force: {}", broke.force);
    assert!(world.get::<Joint2D>(swinging).is_none(), "and removed");
    assert!(world.get::<Joint2D>(hanging).is_some(), "the other holds");

    // A game reacting to the break, despawning what broke, and a body
    // despawned from under a joint still holding: the steps go on.
    world.despawn(swinging);
    world.despawn(hanging);
    for _ in 0..10 {
        physics.step_once(&mut world, G);
        assert!(physics.broken_joints().is_empty());
    }
}

#[test]
fn a_joint_to_a_body_despawned_between_steps_joins_nothing() {
    let mut physics = PhysicsWorld::new();
    let mut world = World::new();
    let a = ball(&mut world, Vec2::ZERO);
    let b = ball(&mut world, Vec2::new(1.0, 0.0));
    let b_id = id(&world, b);
    world.insert(
        a,
        Joint2D {
            b: Some(b_id),
            kind: JointKind::Wheel {
                axis: Vec2::Y,
                spring: Some(RestSpring::default()),
                limit: None,
                motor: Some(Motor::default()),
            },
            break_force: Some(1.0),
            ..Joint2D::default()
        },
    );
    physics.step_once(&mut world, G);
    world.despawn(b);
    for _ in 0..10 {
        physics.step_once(&mut world, G);
    }
    assert!(physics.broken_joints().is_empty(), "nothing left to break");
}

#[test]
fn a_weld_breaks_past_its_torque() {
    let held = |limit: f32| {
        let mut physics = PhysicsWorld::new();
        let mut world = World::new();
        // A two-metre kilogram cantilever: 10 N·m at its root.
        let beam = plank(&mut world, Vec2::new(1.0, 0.0), Vec2::new(1.0, 0.1), 1.0);
        world.insert(
            beam,
            Joint2D {
                anchor_a: Vec2::new(-1.0, 0.0),
                anchor_b: Vec2::ZERO,
                kind: JointKind::Weld {
                    reference_angle: 0.0,
                },
                break_torque: Some(limit),
                ..Joint2D::default()
            },
        );
        let mut broke = 0;
        for _ in 0..60 {
            physics.step_once(&mut world, G);
            broke += physics.broken_joints().len();
        }
        broke
    };
    assert_eq!(held(5.0), 1, "five newton-metres cannot hold ten");
    assert_eq!(held(20.0), 0, "twenty can");
}
