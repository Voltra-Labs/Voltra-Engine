//! The tests of `body`.

use super::*;
use crate::integrate::{integrate_positions, integrate_velocities};

const H: f32 = 1.0 / 60.0;
const G: Vec2 = Vec2::new(0.0, -10.0);
const MAX_ROTATION: f32 = 0.25 * std::f32::consts::PI;

fn spawn(world: &mut World, body: Option<RigidBody>) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform::default());
    if let Some(body) = body {
        world.insert(entity, body);
    }
    entity
}

/// One dynamic body of `mass`, wearing `collider`, gathered.
fn gather_one(collider: Collider, mass: f32) -> SolverBodies {
    let mut world = World::new();
    let entity = spawn(&mut world, Some(RigidBody::new_dynamic(mass)));
    world.insert(entity, collider);
    SolverBodies::gather(&world)
}

#[test]
fn a_box_body_has_the_inertia_of_a_box() {
    // I = m(hx² + hy²)/3, so its inverse is 3·inv_mass/(hx² + hy²).
    let bodies = gather_one(
        Collider::Box {
            half_extents: Vec2::new(2.0, 1.0),
        },
        4.0,
    );

    let expected = 3.0 * 0.25 / (4.0 + 1.0);
    assert!(
        (bodies.get(0).inverse_inertia - expected).abs() < 1e-6,
        "{}",
        bodies.get(0).inverse_inertia
    );
}

#[test]
fn a_circle_body_has_the_inertia_of_a_disc() {
    // I = m·r²/2.
    let bodies = gather_one(Collider::Circle { radius: 2.0 }, 4.0);

    let expected = 2.0 * 0.25 / 4.0;
    assert!(
        (bodies.get(0).inverse_inertia - expected).abs() < 1e-6,
        "{}",
        bodies.get(0).inverse_inertia
    );
}

#[test]
fn the_collider_scale_reaches_the_inertia() {
    // Inertia goes as the square of the extent, so twice the scale is four
    // times as hard to turn. A body that resists like the unscaled shape is
    // a bug that looks like a mass bug.
    let mut world = World::new();
    let entity = spawn(&mut world, Some(RigidBody::new_dynamic(1.0)));
    world.insert(
        entity,
        Collider::Box {
            half_extents: Vec2::splat(1.0),
        },
    );
    world.insert(entity, Transform::default().with_scale(Vec2::splat(2.0)));

    let scaled = SolverBodies::gather(&world).get(0).inverse_inertia;
    let unscaled = gather_one(
        Collider::Box {
            half_extents: Vec2::splat(1.0),
        },
        1.0,
    )
    .get(0)
    .inverse_inertia;

    assert!(
        (scaled - unscaled / 4.0).abs() < 1e-6,
        "{scaled} {unscaled}"
    );
}

#[test]
fn an_immovable_body_has_no_inverse_inertia() {
    // Zero reads as infinite inertia here, exactly as it does for mass.
    let bodies = gather_one(Collider::Circle { radius: 1.0 }, 0.0);
    assert_eq!(bodies.get(0).inverse_inertia, 0.0);

    let mut world = World::new();
    let wall = spawn(&mut world, Some(RigidBody::new_static()));
    world.insert(wall, Collider::Circle { radius: 1.0 });
    assert_eq!(SolverBodies::gather(&world).get(0).inverse_inertia, 0.0);
}

#[test]
fn a_locked_body_has_no_inverse_inertia_and_no_spin() {
    let mut world = World::new();
    let entity = spawn(
        &mut world,
        Some(RigidBody {
            lock_rotation: true,
            angular_velocity: 5.0,
            ..RigidBody::new_dynamic(1.0)
        }),
    );
    world.insert(entity, Collider::Circle { radius: 1.0 });

    let bodies = SolverBodies::gather(&world);

    assert_eq!(bodies.get(0).inverse_inertia, 0.0);
    assert_eq!(bodies.get(0).angular_velocity, 0.0);
}

#[test]
fn a_body_without_a_collider_has_no_inverse_inertia() {
    let mut world = World::new();
    spawn(&mut world, Some(RigidBody::new_dynamic(1.0)));

    assert_eq!(SolverBodies::gather(&world).get(0).inverse_inertia, 0.0);
}

#[test]
fn a_degenerate_collider_does_not_divide_by_zero() {
    // 3·inv_mass/0 is an infinity, and an infinite inverse inertia spins a
    // body out of the world on the first contact it takes.
    let bodies = gather_one(
        Collider::Box {
            half_extents: Vec2::ZERO,
        },
        1.0,
    );

    assert_eq!(bodies.get(0).inverse_inertia, 0.0);
}

#[test]
fn a_spin_is_scattered_back_to_the_component_and_the_transform() {
    let mut world = World::new();
    let entity = spawn(&mut world, Some(RigidBody::new_dynamic(1.0)));
    world.insert(entity, Collider::Circle { radius: 1.0 });

    let mut bodies = SolverBodies::gather(&world);
    for body in bodies.iter_mut() {
        body.angular_velocity = 2.0;
    }
    integrate_positions(&mut bodies, 0.5);
    bodies.scatter(&mut world);

    let rotation = world
        .get::<Transform>(entity)
        .expect("the transform")
        .rotation;
    let spin = world
        .get::<RigidBody>(entity)
        .expect("the body")
        .angular_velocity;

    assert!((rotation - 1.0).abs() < 1e-6, "{rotation}");
    assert!((spin - 2.0).abs() < 1e-6, "{spin}");
}

#[test]
fn a_collider_without_a_body_is_gathered_as_immovable() {
    let mut world = World::new();
    let wall = world.spawn();
    world.insert(wall, Transform::default());
    world.insert(wall, Collider::Circle { radius: 1.0 });

    let bodies = SolverBodies::gather(&world);
    let index = bodies.index_of(wall).expect("gathered");

    assert_eq!(bodies.get(index).inverse_mass, 0.0);
    assert_eq!(bodies.get(index).velocity, Vec2::ZERO);
}

#[test]
fn a_body_with_a_collider_is_gathered_once() {
    let mut world = World::new();
    let entity = spawn(&mut world, Some(RigidBody::new_dynamic(1.0)));
    world.insert(entity, Collider::Circle { radius: 1.0 });

    let bodies = SolverBodies::gather(&world);

    assert_eq!(bodies.len(), 1);
    assert!(
        bodies.get(0).inverse_mass > 0.0,
        "the body won, not the hull"
    );
}

#[test]
fn a_kinematic_body_moves_itself_but_cannot_be_pushed() {
    let mut world = World::new();
    let platform = spawn(
        &mut world,
        Some(RigidBody {
            body_type: BodyType::Kinematic,
            velocity: Vec2::new(2.0, 0.0),
            ..Default::default()
        }),
    );

    let mut bodies = SolverBodies::gather(&world);
    let index = bodies.index_of(platform).expect("gathered");
    assert_eq!(bodies.get(index).inverse_mass, 0.0, "immovable by contacts");

    integrate_velocities(&mut bodies, G, H, MAX_ROTATION);
    integrate_positions(&mut bodies, H);

    assert_eq!(
        bodies.get(index).velocity,
        Vec2::new(2.0, 0.0),
        "gravity must not touch a kinematic body"
    );
    assert!(bodies.get(index).delta_position.x > 0.0, "it still moves");
}

#[test]
fn a_static_body_is_never_moved_by_integration() {
    let mut world = World::new();
    let mut body = RigidBody::new_static();
    body.velocity = Vec2::new(100.0, 100.0);
    let entity = spawn(&mut world, Some(body));

    let mut bodies = SolverBodies::gather(&world);
    integrate_velocities(&mut bodies, G, H, MAX_ROTATION);
    integrate_positions(&mut bodies, H);
    bodies.scatter(&mut world);

    assert_eq!(
        world
            .get::<Transform>(entity)
            .expect("transform")
            .translation,
        Vec2::ZERO
    );
}

#[test]
fn velocity_is_integrated_before_position() {
    // Semi-implicit Euler, unchanged from 11b-1: after one step from rest
    // the body has moved by exactly g·h², and explicit Euler would leave it
    // at zero. The difference is the order of two lines and a stack of
    // boxes climbing on its own.
    let mut world = World::new();
    let entity = spawn(&mut world, Some(RigidBody::new_dynamic(1.0)));

    let mut bodies = SolverBodies::gather(&world);
    integrate_velocities(&mut bodies, G, H, MAX_ROTATION);
    integrate_positions(&mut bodies, H);

    let index = bodies.index_of(entity).expect("gathered");
    let expected = G.y * H * H;
    let actual = bodies.get(index).delta_position.y;
    assert!(
        (actual - expected).abs() < 1e-9,
        "got {actual}, expected {expected} — explicit Euler would give 0"
    );
}

#[test]
fn gravity_scale_zero_makes_a_body_float() {
    let mut world = World::new();
    let entity = spawn(
        &mut world,
        Some(RigidBody {
            gravity_scale: 0.0,
            ..RigidBody::new_dynamic(1.0)
        }),
    );

    let mut bodies = SolverBodies::gather(&world);
    integrate_velocities(&mut bodies, G, H, MAX_ROTATION);
    integrate_positions(&mut bodies, H);

    let index = bodies.index_of(entity).expect("gathered");
    assert_eq!(bodies.get(index).delta_position, Vec2::ZERO);
}

#[test]
fn enormous_damping_stops_a_body_rather_than_reversing_it() {
    // `v *= 1 − damping·h` goes negative once damping·h passes one, which
    // turns a brake into a catapult. A scene file can say 10 000.
    let mut world = World::new();
    let entity = spawn(
        &mut world,
        Some(RigidBody {
            velocity: Vec2::new(10.0, 0.0),
            gravity_scale: 0.0,
            linear_damping: 10_000.0,
            ..RigidBody::new_dynamic(1.0)
        }),
    );

    let mut bodies = SolverBodies::gather(&world);
    integrate_velocities(&mut bodies, Vec2::ZERO, H, MAX_ROTATION);

    let index = bodies.index_of(entity).expect("gathered");
    assert!(bodies.get(index).velocity.x >= 0.0);
}

/// One free dynamic body with the given spin and angular damping.
fn spinning(spin: f32, angular_damping: f32) -> (World, Entity) {
    let mut world = World::new();
    let entity = spawn(
        &mut world,
        Some(RigidBody {
            angular_velocity: spin,
            gravity_scale: 0.0,
            angular_damping,
            ..RigidBody::new_dynamic(1.0)
        }),
    );
    world.insert(entity, Collider::Circle { radius: 1.0 });
    (world, entity)
}

#[test]
fn a_spinning_body_accumulates_a_delta_rotation() {
    let (world, entity) = spinning(2.0, 0.0);
    let mut bodies = SolverBodies::gather(&world);

    integrate_positions(&mut bodies, 0.5);

    let index = bodies.index_of(entity).expect("gathered");
    assert!((bodies.get(index).delta_rotation - 1.0).abs() < 1e-6);
}

#[test]
fn gravity_applies_no_torque() {
    // Gravity acts through the centre of mass. A body that starts spinning
    // when it is dropped is a sign the arm is being taken from the wrong
    // origin.
    let (world, entity) = spinning(0.0, 0.0);
    let mut bodies = SolverBodies::gather(&world);

    integrate_velocities(&mut bodies, G, H, MAX_ROTATION);

    let index = bodies.index_of(entity).expect("gathered");
    assert_eq!(bodies.get(index).angular_velocity, 0.0);
}

#[test]
fn angular_damping_slows_a_spin() {
    let (world, entity) = spinning(4.0, 2.0);
    let mut bodies = SolverBodies::gather(&world);

    integrate_velocities(&mut bodies, Vec2::ZERO, H, MAX_ROTATION);

    let index = bodies.index_of(entity).expect("gathered");
    let spin = bodies.get(index).angular_velocity;
    assert!(spin < 4.0 && spin > 0.0, "{spin}");
}

#[test]
fn a_large_angular_damping_does_not_reverse_the_spin() {
    // The linear twin of this bug is already pinned: `1 − damping·h` goes
    // negative and the brake becomes a catapult, in the other direction.
    let (world, entity) = spinning(4.0, 10_000.0);
    let mut bodies = SolverBodies::gather(&world);

    integrate_velocities(&mut bodies, Vec2::ZERO, H, MAX_ROTATION);

    let index = bodies.index_of(entity).expect("gathered");
    assert!(bodies.get(index).angular_velocity >= 0.0);
}

#[test]
fn a_kinematic_body_keeps_the_spin_it_was_given() {
    let mut world = World::new();
    let entity = spawn(
        &mut world,
        Some(RigidBody {
            body_type: BodyType::Kinematic,
            angular_velocity: 3.0,
            angular_damping: 5.0,
            ..RigidBody::default()
        }),
    );
    world.insert(entity, Collider::Circle { radius: 1.0 });

    let mut bodies = SolverBodies::gather(&world);
    integrate_velocities(&mut bodies, G, H, MAX_ROTATION);

    let index = bodies.index_of(entity).expect("gathered");
    assert_eq!(bodies.get(index).angular_velocity, 3.0);
}

#[test]
fn an_absurd_spin_is_capped_to_a_quarter_turn_per_sub_step() {
    // Past this a body turns its contact face out from under the normal the
    // whole step is solved against.
    let (world, entity) = spinning(1_000.0, 0.0);
    let mut bodies = SolverBodies::gather(&world);

    integrate_velocities(&mut bodies, Vec2::ZERO, H, MAX_ROTATION);

    let index = bodies.index_of(entity).expect("gathered");
    let spin = bodies.get(index).angular_velocity;
    assert!((spin - MAX_ROTATION / H).abs() < 1e-3, "{spin}");
    assert!(spin * H <= MAX_ROTATION + 1e-6, "{spin}");
}

#[test]
fn a_negative_inverse_mass_cannot_be_pushed_backwards() {
    // A scene file is external input, and a negative inverse mass would
    // send a body *towards* whatever pushed it.
    let mut world = World::new();
    let entity = spawn(
        &mut world,
        Some(RigidBody {
            inverse_mass: -1.0,
            ..RigidBody::new_dynamic(1.0)
        }),
    );

    let bodies = SolverBodies::gather(&world);
    let index = bodies.index_of(entity).expect("gathered");

    assert_eq!(bodies.get(index).inverse_mass, 0.0);
}

#[test]
fn scatter_writes_velocity_and_the_accumulated_move_back() {
    let mut world = World::new();
    let entity = spawn(&mut world, Some(RigidBody::new_dynamic(1.0)));

    let mut bodies = SolverBodies::gather(&world);
    integrate_velocities(&mut bodies, G, H, MAX_ROTATION);
    integrate_positions(&mut bodies, H);
    bodies.scatter(&mut world);

    assert!(
        world
            .get::<Transform>(entity)
            .expect("transform")
            .translation
            .y
            < 0.0,
        "it should have moved down"
    );
    assert!(world.get::<RigidBody>(entity).expect("body").velocity.y < 0.0);
}

#[test]
fn a_body_without_a_transform_simulates_and_moves_nothing() {
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, RigidBody::new_dynamic(1.0));

    let mut bodies = SolverBodies::gather(&world);
    integrate_velocities(&mut bodies, G, H, MAX_ROTATION);
    integrate_positions(&mut bodies, H);
    bodies.scatter(&mut world);

    assert!(world.get::<Transform>(entity).is_none());
    assert!(
        world.get::<RigidBody>(entity).expect("body").velocity.y < 0.0,
        "it should still accelerate; only the move is skipped"
    );
}

#[test]
fn an_empty_world_gathers_nothing() {
    assert!(SolverBodies::gather(&World::new()).is_empty());
}

#[test]
fn a_pair_hands_out_both_ends() {
    let mut world = World::new();
    let first = spawn(&mut world, Some(RigidBody::new_dynamic(1.0)));
    let second = spawn(&mut world, Some(RigidBody::new_dynamic(2.0)));

    let mut bodies = SolverBodies::gather(&world);
    let (a, b) = bodies.pair_mut(bodies_index(&bodies, first), bodies_index(&bodies, second));

    assert_eq!(a.entity, first);
    assert_eq!(b.entity, second);
}

#[test]
fn a_pair_in_either_order_names_the_same_bodies() {
    let mut world = World::new();
    let first = spawn(&mut world, Some(RigidBody::new_dynamic(1.0)));
    let second = spawn(&mut world, Some(RigidBody::new_dynamic(2.0)));

    let mut bodies = SolverBodies::gather(&world);
    let (i, j) = (bodies_index(&bodies, first), bodies_index(&bodies, second));
    let (a, b) = bodies.pair_mut(j, i);

    assert_eq!(a.entity, second);
    assert_eq!(b.entity, first);
}

fn bodies_index(bodies: &SolverBodies, entity: Entity) -> usize {
    bodies.index_of(entity).expect("gathered")
}
