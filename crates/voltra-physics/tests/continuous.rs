//! Fast bodies stopped where they first meet something instead of passing
//! through it in one step: against walls always, against moving bodies
//! when they are bullets.

mod common;

use common::*;
use voltra_ecs::{Entity, World};
use voltra_physics::PhysicsWorld;
use voltra_render::glam::Vec2;
use voltra_scene::{
    Collider, CollisionLayers, ConvexPolygon, OneWay, PhysicsMaterial, RigidBody, Transform,
};

/// A static wall 0.1 thick, its left face at x = 5.
fn wall(world: &mut World) -> Entity {
    spawn_box(world, Vec2::new(5.05, 0.0), Vec2::new(0.05, 5.0), None)
}

fn shot(world: &mut World, collider: Collider, velocity: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform::default());
    world.insert(entity, collider);
    let mut body = RigidBody::new_dynamic(1.0);
    body.velocity = velocity;
    body.gravity_scale = 0.0;
    world.insert(entity, body);
    entity
}

fn steps(world: &mut World, physics: &mut PhysicsWorld, count: usize) {
    for _ in 0..count {
        physics.step_once(world, Vec2::ZERO);
    }
}

fn shapes() -> [(&'static str, Collider); 4] {
    [
        (
            "box",
            Collider::Box {
                half_extents: Vec2::splat(0.25),
            },
        ),
        ("circle", Collider::Circle { radius: 0.25 }),
        (
            "capsule",
            Collider::Capsule {
                half_height: 0.5,
                radius: 0.25,
            },
        ),
        (
            "polygon",
            Collider::Polygon {
                points: ConvexPolygon::new(&[
                    Vec2::new(0.25, 0.0),
                    Vec2::new(-0.25, 0.25),
                    Vec2::new(-0.25, -0.25),
                ])
                .expect("a triangle"),
                radius: 0.0,
            },
        ),
    ]
}

#[test]
fn nothing_at_200_units_a_second_passes_a_wall_a_tenth_thick() {
    for (name, shape) in shapes() {
        let mut world = World::new();
        wall(&mut world);
        let fast = shot(&mut world, shape, Vec2::new(200.0, 0.0));
        let mut physics = PhysicsWorld::new();

        for _ in 0..60 {
            physics.step_once(&mut world, Vec2::ZERO);
            let x = translation(&world, fast).x;
            assert!(x < 5.0, "{name} passed the wall: at {x}");
        }
        let v = velocity(&world, fast);
        assert!(v.x <= 1e-3, "{name} is no longer going into it: {v}");
    }
}

#[test]
fn without_continuous_collision_it_tunnels() {
    // What the sweep is for: one step carries the body from one side to the
    // other, and the step's single collide never sees an overlap.
    let mut world = World::new();
    wall(&mut world);
    let fast = shot(
        &mut world,
        Collider::Circle { radius: 0.25 },
        Vec2::new(200.0, 0.0),
    );
    let mut physics = PhysicsWorld::new();
    physics.params_mut().continuous = false;
    steps(&mut world, &mut physics, 60);

    assert!(
        translation(&world, fast).x > 5.1,
        "{}",
        translation(&world, fast)
    );
}

#[test]
fn a_fast_ball_bounces_off_a_thin_wall_as_a_slow_one_would() {
    let mut world = World::new();
    let barrier = wall(&mut world);
    let bouncy = PhysicsMaterial {
        friction: 0.0,
        restitution: 1.0,
    };
    world.insert(barrier, bouncy);
    let fast = shot(
        &mut world,
        Collider::Circle { radius: 0.25 },
        Vec2::new(200.0, 0.0),
    );
    world.insert(fast, bouncy);
    let mut physics = PhysicsWorld::new();
    steps(&mut world, &mut physics, 10);

    let v = velocity(&world, fast);
    assert!(v.x < -150.0, "it came back: {v}");
    assert!(translation(&world, fast).x < 5.0);
}

#[test]
fn a_fast_fall_lands_on_the_floor() {
    let mut world = World::new();
    spawn_box(
        &mut world,
        Vec2::new(0.0, -0.05),
        Vec2::new(5.0, 0.05),
        None,
    );
    let fast = shot(
        &mut world,
        Collider::Box {
            half_extents: Vec2::splat(0.25),
        },
        Vec2::new(0.0, -300.0),
    );
    world
        .get_mut::<Transform>(fast)
        .expect("placed")
        .translation = Vec2::new(0.0, 10.0);
    world
        .get_mut::<RigidBody>(fast)
        .expect("a body")
        .gravity_scale = 1.0;
    let mut physics = PhysicsWorld::new();
    for _ in 0..180 {
        physics.step_once(&mut world, G);
    }
    let y = translation(&world, fast).y;
    assert!((y - 0.25).abs() < 0.03, "resting on the floor: {y}");
}

#[test]
fn a_spinning_bar_does_not_sweep_through_a_wall() {
    let mut world = World::new();
    wall(&mut world);
    let bar = shot(
        &mut world,
        Collider::Box {
            half_extents: Vec2::new(5.5, 0.05),
        },
        Vec2::ZERO,
    );
    world.get_mut::<Transform>(bar).expect("placed").rotation = std::f32::consts::FRAC_PI_2;
    world
        .get_mut::<RigidBody>(bar)
        .expect("a body")
        .angular_velocity = -60.0;
    let mut physics = PhysicsWorld::new();

    for _ in 0..30 {
        physics.step_once(&mut world, Vec2::ZERO);
        let turn = rotation(&world, bar);
        let centre = translation(&world, bar);
        // The bar's ends, 5.5 either side of its centre: neither may be
        // past the wall's far face within its height.
        let reach = Vec2::from_angle(turn) * 5.5;
        for tip in [centre + reach, centre - reach] {
            assert!(
                !(tip.x > 5.1 && tip.y.abs() < 4.9),
                "an end went through the wall at turn {turn}: {tip}"
            );
        }
        // And it is not inside it either, beyond the depth a contact
        // resolves.
        for tip in [centre + reach, centre - reach] {
            assert!(
                !(tip.x > 5.02 && tip.y.abs() < 4.9),
                "{tip} sank into the wall"
            );
        }
    }
}

#[test]
fn a_bullet_stops_at_a_thin_moving_body_and_a_plain_body_does_not() {
    for bullet in [true, false] {
        let mut world = World::new();
        let plank = spawn_box(
            &mut world,
            Vec2::new(5.05, 0.0),
            Vec2::new(0.05, 1.0),
            Some(1.0),
        );
        world
            .get_mut::<RigidBody>(plank)
            .expect("a body")
            .gravity_scale = 0.0;
        let fast = shot(
            &mut world,
            Collider::Circle { radius: 0.1 },
            Vec2::new(200.0, 0.0),
        );
        world.get_mut::<RigidBody>(fast).expect("a body").bullet = bullet;
        let mut physics = PhysicsWorld::new();
        steps(&mut world, &mut physics, 10);

        let x = translation(&world, fast).x;
        if bullet {
            assert!(
                x < translation(&world, plank).x,
                "the bullet stayed behind the plank: {x}"
            );
            assert!(
                velocity(&world, plank).x > 1.0,
                "and pushed it: {}",
                velocity(&world, plank)
            );
        } else {
            // Box2D's default, kept: only static geometry stops a body that
            // is not a bullet, so a crowd of fast bodies stays cheap.
            assert!(x > 10.0, "a plain body passes a thin moving one: {x}");
        }
    }
}

#[test]
fn a_fast_body_falls_onto_a_one_way_platform_and_rises_through_it() {
    let mut world = World::new();
    let platform = spawn_box(
        &mut world,
        Vec2::new(0.0, -0.05),
        Vec2::new(3.0, 0.05),
        None,
    );
    world.insert(platform, OneWay::default());
    let falling = shot(
        &mut world,
        Collider::Circle { radius: 0.25 },
        Vec2::new(0.0, -300.0),
    );
    world
        .get_mut::<Transform>(falling)
        .expect("placed")
        .translation = Vec2::new(0.0, 8.0);
    let rising = shot(
        &mut world,
        Collider::Circle { radius: 0.25 },
        Vec2::new(0.0, 300.0),
    );
    world
        .get_mut::<Transform>(rising)
        .expect("placed")
        .translation = Vec2::new(2.0, -8.0);
    let mut physics = PhysicsWorld::new();
    steps(&mut world, &mut physics, 10);

    assert!(translation(&world, falling).y > 0.0, "caught from above");
    assert!(
        translation(&world, rising).y > 8.0,
        "let through from below"
    );
}

#[test]
fn a_wall_on_a_layer_the_body_does_not_see_is_passed() {
    let mut world = World::new();
    let barrier = wall(&mut world);
    world.insert(
        barrier,
        CollisionLayers {
            layers: CollisionLayers::bit(1),
            mask: CollisionLayers::ALL,
        },
    );
    let fast = shot(
        &mut world,
        Collider::Circle { radius: 0.25 },
        Vec2::new(200.0, 0.0),
    );
    world.insert(
        fast,
        CollisionLayers {
            layers: CollisionLayers::bit(0),
            mask: CollisionLayers::bit(0),
        },
    );
    let mut physics = PhysicsWorld::new();
    steps(&mut world, &mut physics, 10);

    assert!(translation(&world, fast).x > 10.0);
}
