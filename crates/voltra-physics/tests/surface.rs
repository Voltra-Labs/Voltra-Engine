//! Friction and restitution: the surface a contact mixes, seen from the scene.

mod common;

use common::*;
use voltra_ecs::World;
use voltra_physics::{step, ImpulseCache, SolverParams};
use voltra_render::glam::Vec2;
use voltra_scene::{PhysicsMaterial, RigidBody};

#[test]
fn friction_stops_a_sliding_body() {
    let mut world = World::new();
    spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(20.0, 1.0), None);
    let crate_ = spawn_box(
        &mut world,
        Vec2::new(0.0, 0.45),
        Vec2::splat(0.5),
        Some(1.0),
    );
    world.get_mut::<RigidBody>(crate_).expect("body").velocity = Vec2::new(10.0, 0.0);

    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 600);

    let speed = velocity(&world, crate_).x;
    assert!(speed.abs() < 0.5, "friction should stop it, got {speed}");
}

#[test]
fn a_frictionless_body_keeps_sliding() {
    let mut world = World::new();
    let floor = spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(20.0, 1.0), None);
    let slippery = PhysicsMaterial {
        friction: 0.0,
        restitution: 0.0,
    };
    world.insert(floor, slippery);
    let puck = spawn_box(
        &mut world,
        Vec2::new(0.0, 0.45),
        Vec2::splat(0.5),
        Some(1.0),
    );
    world.insert(puck, slippery);
    world.get_mut::<RigidBody>(puck).expect("body").velocity = Vec2::new(10.0, 0.0);

    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 300);

    let speed = velocity(&world, puck).x;
    assert!(speed > 9.5, "nothing should slow it, got {speed}");
}

#[test]
fn a_bouncy_body_returns_most_of_its_drop() {
    let mut world = World::new();
    let floor = spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(20.0, 1.0), None);
    let bouncy = PhysicsMaterial {
        friction: 0.6,
        restitution: 0.9,
    };
    world.insert(floor, bouncy);
    let ball = spawn_box(&mut world, Vec2::new(0.0, 5.0), Vec2::splat(0.5), Some(1.0));
    world.insert(ball, bouncy);

    let mut cache = ImpulseCache::default();
    let params = SolverParams::default();
    let mut bounced = false;
    let mut highest: f32 = -10.0;
    for _ in 0..600 {
        step(&mut world, &mut cache, &params, G, DT);
        if velocity(&world, ball).y > 1.0 {
            bounced = true;
        }
        if bounced {
            highest = highest.max(translation(&world, ball).y);
        }
    }

    assert!(bounced, "restitution 0.9 must send it back up");
    assert!(
        highest > 2.0,
        "it should return most of the drop: {highest}"
    );
}

#[test]
fn a_dull_body_does_not_bounce() {
    let mut world = World::new();
    spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(20.0, 1.0), None);
    let ball = spawn_box(&mut world, Vec2::new(0.0, 5.0), Vec2::splat(0.5), Some(1.0));

    let mut cache = ImpulseCache::default();
    let params = SolverParams::default();
    let mut highest_after_landing: f32 = -10.0;
    let mut landed = false;
    for _ in 0..600 {
        step(&mut world, &mut cache, &params, G, DT);
        if translation(&world, ball).y < 0.6 {
            landed = true;
        }
        if landed {
            highest_after_landing = highest_after_landing.max(translation(&world, ball).y);
        }
    }

    assert!(landed);
    assert!(
        highest_after_landing < 0.7,
        "restitution 0 must not bounce: {highest_after_landing}"
    );
}

#[test]
fn a_resting_body_does_not_bounce_on_its_own_noise() {
    // Restitution below the threshold speed is discarded. Without that, a
    // perfectly elastic body vibrates on the floor forever.
    let mut world = World::new();
    let floor = spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(20.0, 1.0), None);
    let elastic = PhysicsMaterial {
        friction: 0.6,
        restitution: 1.0,
    };
    world.insert(floor, elastic);
    let ball = spawn_box(
        &mut world,
        Vec2::new(0.0, 0.49),
        Vec2::splat(0.5),
        Some(1.0),
    );
    world.insert(ball, elastic);

    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 600);

    let speed = velocity(&world, ball).y;
    assert!(speed.abs() < 1.0, "it must settle, got {speed}");
}
