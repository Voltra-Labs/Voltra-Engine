//! Scene builders and readers the behaviour tests share.
//!
//! Integration-test binaries each compile this module separately, so a helper
//! only one of them needs would otherwise be dead code in the others.
#![allow(dead_code)]

use voltra_ecs::{Entity, World};
use voltra_physics::{step, ImpulseCache, SolverParams};
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, PhysicsMaterial, RigidBody, Transform};

pub const DT: f32 = 1.0 / 60.0;
pub const G: Vec2 = Vec2::new(0.0, -10.0);

/// A box, static when `mass` is `None` and dynamic otherwise.
pub fn spawn_box(world: &mut World, at: Vec2, half_extents: Vec2, mass: Option<f32>) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform::from_translation(at));
    world.insert(entity, Collider::Box { half_extents });
    if let Some(mass) = mass {
        world.insert(entity, RigidBody::new_dynamic(mass));
    }
    entity
}

/// A static floor whose top face is at y = -1.5, and a ball of radius 0.5
/// dropped from the origin. Its resting centre is therefore y = -1.
pub fn floor_and_ball() -> (World, Entity, Entity) {
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

    (world, floor, ball)
}

pub fn translation(world: &World, entity: Entity) -> Vec2 {
    world
        .get::<Transform>(entity)
        .expect("the transform is there")
        .translation
}

pub fn velocity(world: &World, entity: Entity) -> Vec2 {
    world
        .get::<RigidBody>(entity)
        .expect("the body is there")
        .velocity
}

/// Runs `count` steps of the default solver.
pub fn run(world: &mut World, cache: &mut ImpulseCache, gravity: Vec2, count: usize) {
    let params = SolverParams::default();
    for _ in 0..count {
        step(world, cache, &params, gravity, DT);
    }
}

/// A box with a rotation, and a mass if it is dynamic.
pub fn spawn_turned_box(
    world: &mut World,
    at: Vec2,
    half_extents: Vec2,
    mass: Option<f32>,
    rotation: f32,
) -> Entity {
    let entity = spawn_box(world, at, half_extents, mass);
    world
        .get_mut::<Transform>(entity)
        .expect("just spawned")
        .rotation = rotation;
    entity
}

pub fn rotation(world: &World, entity: Entity) -> f32 {
    world
        .get::<Transform>(entity)
        .expect("the transform is there")
        .rotation
}

pub fn spin(world: &World, entity: Entity) -> f32 {
    world
        .get::<RigidBody>(entity)
        .expect("the body is there")
        .angular_velocity
}

/// A static ramp at `angle`, and a box resting on its surface.
pub fn ramp(angle: f32, friction: f32) -> (World, Entity) {
    let mut world = World::new();
    let slope = spawn_turned_box(
        &mut world,
        Vec2::new(0.0, -1.0),
        Vec2::new(5.0, 0.5),
        None,
        angle,
    );
    world.insert(
        slope,
        PhysicsMaterial {
            friction,
            restitution: 0.0,
        },
    );

    let up = Vec2::from_angle(angle).rotate(Vec2::new(0.0, 0.98));
    let crate_ = spawn_turned_box(
        &mut world,
        Vec2::new(0.0, -1.0) + up,
        Vec2::splat(0.5),
        Some(1.0),
        angle,
    );
    world.insert(
        crate_,
        PhysicsMaterial {
            friction,
            restitution: 0.0,
        },
    );

    (world, crate_)
}
