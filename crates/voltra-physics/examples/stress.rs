//! Times the physics step on scenes past the O(n²) broad phase's comfort.
//!
//! `cargo run --release -p voltra-physics --example stress`
//!
//! Two scenes, each stepped for ten simulated seconds at 60 Hz:
//!
//! - **level**: a floor of 2 000 static tiles, as a tilemap generates them
//!   one box per run, with 150 crates dropped in piles on it;
//! - **pyramid**: a 20-row pyramid of 210 boxes settling on one floor;
//! - **shots**: 100 balls fired at 150 units a second at a wall a tenth
//!   thick, every one of them fast — the continuous sweep's worst case.
//!
//! Each runs with continuous collision on and off, so its cost is read off
//! the same run.
//!
//! Reports the mean milliseconds per step over the first and the last second,
//! so settling — and, with sleeping, going to sleep — shows up as the
//! difference. Not a test: a timing on a shared machine is not a pass or a
//! fail. The deterministic counts the tests assert on are what keep it honest.

use std::time::Instant;

use voltra_ecs::World;
use voltra_physics::PhysicsWorld;
use voltra_render::glam::Vec2;
use voltra_scene::{Collider2D, RigidBody2D, Transform2D};

const STEP: f32 = 1.0 / 60.0;
const STEPS: usize = 600;
const GRAVITY: Vec2 = Vec2::new(0.0, -10.0);

fn floor(world: &mut World, at: Vec2, half: Vec2) {
    let entity = world.spawn();
    world.insert(entity, Transform2D::from_translation(at));
    world.insert(entity, Collider2D::Box { half_extents: half });
}

fn crate_at(world: &mut World, at: Vec2) {
    let entity = world.spawn();
    world.insert(entity, Transform2D::from_translation(at));
    world.insert(entity, RigidBody2D::new_dynamic(1.0));
    world.insert(
        entity,
        Collider2D::Box {
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

/// A wall a tenth thick at x = 10, and a column of balls fired at it.
fn shots() -> World {
    let mut world = World::new();
    floor(&mut world, Vec2::new(10.05, 0.0), Vec2::new(0.05, 60.0));
    for k in 0..100 {
        let entity = world.spawn();
        world.insert(
            entity,
            Transform2D::from_translation(Vec2::new(1.3, k as f32 * 1.1 - 55.0)),
        );
        let mut body = RigidBody2D::new_dynamic(1.0);
        body.velocity = Vec2::new(150.0, 0.0);
        body.gravity_scale = 0.0;
        world.insert(entity, body);
        world.insert(entity, Collider2D::Circle { radius: 0.25 });
    }
    world
}

fn run(name: &str, build: fn() -> World) {
    for continuous in [true, false] {
        let label = if continuous {
            name.to_string()
        } else {
            format!("{name} (discrete)")
        };
        step_all(&label, build(), continuous);
    }
}

fn step_all(name: &str, mut world: World, continuous: bool) {
    let mut physics = PhysicsWorld::with_step(STEP);
    physics.params_mut().continuous = continuous;
    let mut times = Vec::with_capacity(STEPS);
    for _ in 0..STEPS {
        let start = Instant::now();
        physics.step_once(&mut world, GRAVITY);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let mean = |slice: &[f64]| slice.iter().sum::<f64>() / slice.len() as f64;
    println!(
        "{name:18} first second {:7.3} ms/step   last second {:7.3} ms/step   contacts {}   asleep {}",
        mean(&times[..60]),
        mean(&times[STEPS - 60..]),
        physics.contacts().len(),
        physics.asleep_count(),
    );
    // The fastest body left: what keeps an island awake.
    let (linear, angular) =
        world
            .query::<RigidBody2D>()
            .fold((0.0f32, 0.0f32), |(l, a), (_, body)| {
                (
                    l.max(body.velocity.length()),
                    a.max(body.angular_velocity.abs()),
                )
            });
    println!(
        "                   fastest body at the end: {linear:.4} u/s, {:.3} deg/s",
        angular.to_degrees()
    );
}

fn main() {
    run("level", level);
    run("pyramid", pyramid);
    run("shots", shots);
}
