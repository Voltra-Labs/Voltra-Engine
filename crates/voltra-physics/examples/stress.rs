//! Times the physics step on scenes past the O(n²) broad phase's comfort.
//!
//! `cargo run --release -p voltra-physics --example stress`
//!
//! Two scenes, each stepped for ten simulated seconds at 60 Hz:
//!
//! - **level**: a floor of 2 000 static tiles, as a tilemap generates them
//!   one box per run, with 150 crates dropped in piles on it;
//! - **pyramid**: a 20-row pyramid of 210 boxes settling on one floor.
//!
//! Reports the mean milliseconds per step over the first and the last second,
//! so settling — and, with sleeping, going to sleep — shows up as the
//! difference. Not a test: a timing on a shared machine is not a pass or a
//! fail. The deterministic counts the tests assert on are what keep it honest.

use std::time::Instant;

use voltra_ecs::World;
use voltra_physics::PhysicsWorld;
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, RigidBody, Transform};

const STEP: f32 = 1.0 / 60.0;
const STEPS: usize = 600;
const GRAVITY: Vec2 = Vec2::new(0.0, -10.0);

fn floor(world: &mut World, at: Vec2, half: Vec2) {
    let entity = world.spawn();
    world.insert(entity, Transform::from_translation(at));
    world.insert(entity, Collider::Box { half_extents: half });
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

fn level() -> World {
    let mut world = World::new();
    for i in 0..2000 {
        floor(
            &mut world,
            Vec2::new(i as f32 - 1000.0, -0.5),
            Vec2::splat(0.5),
        );
    }
    for pile in 0..15 {
        for k in 0..10 {
            crate_at(
                &mut world,
                Vec2::new(pile as f32 * 40.0 - 300.0, 1.0 + k as f32 * 1.05),
            );
        }
    }
    world
}

fn pyramid() -> World {
    let mut world = World::new();
    floor(&mut world, Vec2::new(0.0, -0.5), Vec2::new(40.0, 0.5));
    let rows = 20;
    for row in 0..rows {
        for column in 0..(rows - row) {
            let x = column as f32 - (rows - row) as f32 * 0.5 + 0.5;
            crate_at(&mut world, Vec2::new(x, 0.5 + row as f32 * 1.0));
        }
    }
    world
}

fn run(name: &str, mut world: World) {
    let mut physics = PhysicsWorld::with_step(STEP);
    let mut times = Vec::with_capacity(STEPS);
    for _ in 0..STEPS {
        let start = Instant::now();
        physics.step_once(&mut world, GRAVITY);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let mean = |slice: &[f64]| slice.iter().sum::<f64>() / slice.len() as f64;
    println!(
        "{name:8} first second {:7.3} ms/step   last second {:7.3} ms/step   contacts at end {}",
        mean(&times[..60]),
        mean(&times[STEPS - 60..]),
        physics.contacts().len(),
    );
}

fn main() {
    run("level", level());
    run("pyramid", pyramid());
}
