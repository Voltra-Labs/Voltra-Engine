//! A joint's part switched off while the scene runs stops acting at once: a
//! motor no longer drives, a spring no longer holds, a limit no longer
//! stops — for a wheel, a slider and a hinge alike. A part's impulse is
//! carried from step to step, so each of these would otherwise go on
//! pushing with its last one for ever.

mod common;

use common::*;
use voltra_ecs::{Entity, World};
use voltra_physics::ImpulseCache;
use voltra_render::glam::Vec2;
use voltra_scene::{
    AngleLimit, Collider2D, Joint2D, JointKind, LinearMotor, Motor, RestSpring, RigidBody2D,
    SceneId, Transform2D, TranslationLimit,
};

/// A dynamic ball of radius 0.2 and a kilogram at the origin, `kind` on it,
/// held to the world there.
fn held_ball(world: &mut World, kind: JointKind) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Transform2D::default());
    world.insert(entity, Collider2D::Circle { radius: 0.2 });
    world.insert(entity, RigidBody2D::new_dynamic(1.0));
    world.insert(
        entity,
        Joint2D {
            kind,
            ..Joint2D::default()
        },
    );
    entity
}

/// A bar a unit long and a kilogram, from the origin to the right, hinged
/// to the world at the origin by `kind`.
fn hinged_bar(world: &mut World, kind: JointKind) -> Entity {
    let bar = spawn_box(world, Vec2::new(0.5, 0.0), Vec2::new(0.5, 0.05), Some(1.0));
    world.insert(
        bar,
        Joint2D {
            anchor_a: Vec2::new(-0.5, 0.0),
            kind,
            ..Joint2D::default()
        },
    );
    bar
}

/// Swaps the kind of `entity`'s joint for `kind`, as an inspector or a
/// game editing the component does.
fn switch(world: &mut World, entity: Entity, kind: JointKind) {
    world.get_mut::<Joint2D>(entity).expect("its joint").kind = kind;
}

/// How far `entity` falls in the half second after `kind` replaces its
/// joint's — a body nothing holds any more falls about 1.25.
fn falls_after(
    world: &mut World,
    cache: &mut ImpulseCache,
    entity: Entity,
    kind: JointKind,
) -> f32 {
    let before = translation(world, entity).y;
    switch(world, entity, kind);
    run(world, cache, G, 30);
    before - translation(world, entity).y
}

/// The lowest a hinged bar turns to in the second after `kind` replaces
/// its joint's: a bar let go swings down past a radian.
fn swings_after(world: &mut World, cache: &mut ImpulseCache, bar: Entity, kind: JointKind) -> f32 {
    switch(world, bar, kind);
    let mut lowest = f32::INFINITY;
    for _ in 0..60 {
        run(world, cache, G, 1);
        lowest = lowest.min(rotation(world, bar));
    }
    lowest
}

/// A motor at a speed it cannot reach in time: it pushes as hard as it may
/// every step, so its impulse is at its cap when it is switched off.
fn saturated() -> Motor {
    Motor {
        speed: 1000.0,
        max_torque: 1.0,
    }
}

fn wheel(
    spring: Option<RestSpring>,
    limit: Option<TranslationLimit>,
    motor: Option<Motor>,
) -> JointKind {
    JointKind::Wheel {
        axis: Vec2::Y,
        spring,
        limit,
        motor,
    }
}

fn slider(
    axis: Vec2,
    spring: Option<RestSpring>,
    limit: Option<TranslationLimit>,
    motor: Option<LinearMotor>,
) -> JointKind {
    JointKind::Prismatic {
        axis,
        reference_angle: 0.0,
        limit,
        motor,
        spring,
    }
}

fn hinge(limit: Option<AngleLimit>, motor: Option<Motor>, spring: Option<RestSpring>) -> JointKind {
    JointKind::Revolute {
        reference_angle: 0.0,
        limit,
        motor,
        spring,
    }
}

const SPRING: RestSpring = RestSpring {
    hertz: 4.0,
    damping_ratio: 1.0,
    rest: 0.0,
};

const SHORT: TranslationLimit = TranslationLimit {
    lower: -0.1,
    upper: 0.1,
};

/// Spins a wheel up on `on`'s motor for a second, switches to `off`, and
/// gives the spin at the switch and a second later.
fn spin_after(on: JointKind, off: JointKind) -> (f32, f32) {
    let mut world = World::new();
    let body = held_ball(&mut world, on);
    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, Vec2::ZERO, 60);
    let at_switch = spin(&world, body);
    switch(&mut world, body, off);
    run(&mut world, &mut cache, Vec2::ZERO, 60);
    (at_switch, spin(&world, body))
}

#[test]
fn a_motor_switched_off_lets_its_wheel_or_hinge_coast() {
    for (name, on, off) in [
        (
            "wheel",
            wheel(None, None, Some(saturated())),
            wheel(None, None, None),
        ),
        (
            "hinge",
            hinge(None, Some(saturated()), None),
            hinge(None, None, None),
        ),
        (
            "wheel, no torque left",
            wheel(None, None, Some(saturated())),
            wheel(
                None,
                None,
                Some(Motor {
                    speed: 1000.0,
                    max_torque: 0.0,
                }),
            ),
        ),
    ] {
        let (at_switch, later) = spin_after(on, off);
        assert!(at_switch > 10.0, "{name}: spun up first: {at_switch}");
        assert!(
            later <= at_switch * 1.01 && later > at_switch * 0.8,
            "{name}: coasts at {at_switch}, not driven on: {later}"
        );
    }
}

#[test]
fn a_slider_motor_switched_off_lets_it_coast() {
    let mut world = World::new();
    let motor = LinearMotor {
        speed: 1000.0,
        max_force: 1.0,
    };
    let body = held_ball(&mut world, slider(Vec2::X, None, None, Some(motor)));
    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, Vec2::ZERO, 60);
    let at_switch = velocity(&world, body).x;
    assert!(at_switch > 0.5, "driven first: {at_switch}");
    switch(&mut world, body, slider(Vec2::X, None, None, None));
    run(&mut world, &mut cache, Vec2::ZERO, 60);
    let later = velocity(&world, body).x;
    assert!(
        (later - at_switch).abs() < 0.02 * at_switch,
        "coasts at {at_switch}: {later}"
    );
}

#[test]
fn a_spring_switched_off_or_slackened_holds_nothing_up() {
    let slack = RestSpring {
        hertz: 0.0,
        ..SPRING
    };
    for (name, on, off) in [
        (
            "wheel",
            wheel(Some(SPRING), None, None),
            wheel(None, None, None),
        ),
        (
            "wheel, no frequency",
            wheel(Some(SPRING), None, None),
            wheel(Some(slack), None, None),
        ),
        (
            "slider",
            slider(Vec2::Y, Some(SPRING), None, None),
            slider(Vec2::Y, None, None, None),
        ),
    ] {
        let mut world = World::new();
        let body = held_ball(&mut world, on);
        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 120);
        let sag = translation(&world, body).y;
        assert!(sag.abs() < 0.05, "{name}: held up by its spring: {sag}");
        let fell = falls_after(&mut world, &mut cache, body, off);
        assert!(fell > 0.8, "{name}: falls once let go: {fell}");
    }
    let mut world = World::new();
    let bar = hinged_bar(&mut world, hinge(None, None, Some(SPRING)));
    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 120);
    let sag = rotation(&world, bar);
    assert!(sag > -0.3, "the hinge's spring holds the bar up: {sag}");
    let lowest = swings_after(&mut world, &mut cache, bar, hinge(None, None, Some(slack)));
    assert!(lowest < -1.0, "and lets it swing down: {lowest}");
}

#[test]
fn a_limit_taken_off_no_longer_stops_anything() {
    for (name, on, off) in [
        (
            "wheel",
            wheel(None, Some(SHORT), None),
            wheel(None, None, None),
        ),
        (
            "slider",
            slider(Vec2::Y, None, Some(SHORT), None),
            slider(Vec2::Y, None, None, None),
        ),
    ] {
        let mut world = World::new();
        let body = held_ball(&mut world, on);
        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 60);
        let rest = translation(&world, body).y;
        assert!(
            (rest + 0.1).abs() < 0.02,
            "{name}: on its lower limit: {rest}"
        );
        let fell = falls_after(&mut world, &mut cache, body, off);
        assert!(fell > 0.8, "{name}: falls once it is gone: {fell}");
    }
    let stop = AngleLimit {
        lower: -0.05,
        upper: 0.05,
    };
    let mut world = World::new();
    let bar = hinged_bar(&mut world, hinge(Some(stop), None, None));
    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 60);
    let rest = rotation(&world, bar);
    assert!(
        (rest + 0.05).abs() < 0.02,
        "the hinge rests on its limit: {rest}"
    );
    let lowest = swings_after(&mut world, &mut cache, bar, hinge(None, None, None));
    assert!(lowest < -1.0, "and swings down without it: {lowest}");
}
