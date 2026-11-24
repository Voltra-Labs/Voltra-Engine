//! Physics steps: many bodies in contact, and a pile settling.

use voltra_ecs::World;
use voltra_physics::PhysicsWorld;
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, RigidBody, Transform};

use super::Bench;
use crate::harness::{iter_batched, measure};

const STEP: f32 = 1.0 / 60.0;
const GRAVITY: Vec2 = Vec2::new(0.0, -10.0);

/// Steps timed per iteration of the 1k-body benchmark, reported per step.
const STEPS: u32 = 10;

/// Steps of the pile benchmark: two simulated seconds, the bulk of its
/// settling. Reported per step.
const SETTLE: u32 = 120;

pub(super) fn benches() -> Vec<Bench> {
    vec![
        Bench::new("physics/step/1k-bodies", 15.0, |config| {
            measure(
                config,
                STEPS,
                iter_batched(columns, |(mut world, mut physics)| {
                    for _ in 0..STEPS {
                        physics.step_once(&mut world, GRAVITY);
                    }
                    (world, physics)
                }),
            )
            .into()
        }),
        Bench::new("physics/pile/settle-210", 15.0, |config| {
            measure(
                config,
                SETTLE,
                iter_batched(pyramid, |(mut world, mut physics)| {
                    for _ in 0..SETTLE {
                        physics.step_once(&mut world, GRAVITY);
                    }
                    (world, physics)
                }),
            )
            .into()
        }),
    ]
}

fn floor(world: &mut World, at: Vec2, half_extents: Vec2) {
    let entity = world.spawn();
    world.insert(entity, Transform::from_translation(at));
    world.insert(entity, Collider::Box { half_extents });
}

fn crate_at(world: &mut World, at: Vec2) {
    let entity = world.spawn();
    world.insert(entity, Transform::from_translation(at));
    world.insert(entity, RigidBody::new_dynamic(1.0));
    world.insert(
        entity,
        Collider::Box {
            half_extents: Vec2::splat(0.5),
        },
    );
}

/// 1 000 boxes in 50 columns of 20, each resting on the one below and the
/// bottom row on a floor: every body awake and in contact from the first
/// step, the solver's working case rather than free fall.
fn columns() -> (World, PhysicsWorld) {
    let mut world = World::new();
    floor(&mut world, Vec2::new(0.0, -0.5), Vec2::new(60.0, 0.5));
    for column in 0..50 {
        for row in 0..20 {
            let x = column as f32 * 1.5 - 37.0;
            crate_at(&mut world, Vec2::new(x, 0.5 + row as f32));
        }
    }
    (world, PhysicsWorld::with_step(STEP))
}

/// A 20-row pyramid of 210 boxes on one floor, from the moment it is
/// placed: the contacts form, the impulses warm up, the pile settles.
fn pyramid() -> (World, PhysicsWorld) {
    let mut world = World::new();
    floor(&mut world, Vec2::new(0.0, -0.5), Vec2::new(40.0, 0.5));
    let rows = 20;
    for row in 0..rows {
        for column in 0..(rows - row) {
            let x = column as f32 - (rows - row) as f32 * 0.5 + 0.5;
            crate_at(&mut world, Vec2::new(x, 0.5 + row as f32));
        }
    }
    (world, PhysicsWorld::with_step(STEP))
}
