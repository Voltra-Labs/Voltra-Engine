//! Constant speed on slopes: walked up and down, the same ground covered
//! across as on the flat.

use super::*;

/// A long slope rising to the right at `degrees`, and a character stood on
/// it at the origin's column, `constant` speed on or off and no snap.
fn on_a_slope(degrees: f32, constant: bool) -> (World, Entity) {
    let mut world = World::new();
    block(
        &mut world,
        Vec2::ZERO,
        Vec2::new(20.0, 0.5),
        degrees.to_radians(),
    );
    let me = character(&mut world, Vec2::new(0.0, 1.5));
    if let Some(body) = world.get_mut::<CharacterBody2D>(me) {
        body.constant_speed_on_slopes = constant;
        body.snap = 0.0;
    }
    run(&mut world, me, Vec2::ZERO, None, 60);
    assert!(body(&world, me).is_on_floor(), "landed on the slope");
    (world, me)
}

/// Walks `ticks` at `walk` across, and answers how far it went across and
/// whether it was on the floor after every move.
fn walk(world: &mut World, me: Entity, walk: f32, ticks: u32) -> (f32, bool, Vec2) {
    let start = at(world, me).x;
    let mut velocity = Vec2::ZERO;
    let mut always = true;
    for _ in 0..ticks {
        velocity = run(world, me, velocity, Some(walk), 1);
        always &= body(world, me).is_on_floor();
    }
    (at(world, me).x - start, always, velocity)
}

#[test]
fn up_a_slope_it_covers_the_ground_it_would_on_the_flat() {
    let (mut world, me) = on_a_slope(30.0, true);
    let (across, always, velocity) = walk(&mut world, me, 2.0, 60);
    assert!((across - 2.0).abs() < 0.02, "{across}");
    assert!(always, "on the slope all the way");
    assert!(
        (velocity.x - 2.0).abs() < 1e-4,
        "its speed kept: {velocity}"
    );
    assert!(velocity.y <= 0.0, "not flung off the top: {velocity}");

    // Off, the part of each step going into the slope is taken off: slower
    // across, by however much the game's velocity kept rising with it.
    let (mut world, me) = on_a_slope(30.0, false);
    let (across, _, velocity) = walk(&mut world, me, 2.0, 60);
    assert!(across < 1.95, "{across}");
    assert!(velocity.y > 0.0, "rising along the slope: {velocity}");
}

#[test]
fn down_a_slope_it_covers_the_same_ground_and_stays_on_it() {
    let (mut world, me) = on_a_slope(30.0, true);
    let (across, always, velocity) = walk(&mut world, me, -3.0, 60);
    assert!((across + 3.0).abs() < 0.02, "{across}");
    assert!(always, "along the slope, with no snap to bring it back");
    assert!((velocity.x + 3.0).abs() < 1e-4, "{velocity}");

    // Off and with no snap, it walks off the slope into the air.
    let (mut world, me) = on_a_slope(30.0, false);
    let (_, always, _) = walk(&mut world, me, -3.0, 60);
    assert!(!always, "it left the floor");
}

#[test]
fn on_the_flat_constant_speed_changes_nothing() {
    let mut world = World::new();
    floor(&mut world);
    let me = character(&mut world, Vec2::new(0.0, 0.3));
    if let Some(body) = world.get_mut::<CharacterBody2D>(me) {
        body.constant_speed_on_slopes = true;
    }
    run(&mut world, me, Vec2::ZERO, None, 30);
    let start = at(&world, me);
    run(&mut world, me, Vec2::ZERO, Some(3.0), 60);
    let walked = at(&world, me) - start;
    assert!((walked.x - 3.0).abs() < 1e-2, "{walked}");
    assert!(walked.y.abs() < 1e-3, "{walked}");
    assert!(body(&world, me).is_on_floor());
}

#[test]
fn a_jump_off_a_slope_is_not_turned_along_it() {
    let (mut world, me) = on_a_slope(30.0, true);
    let start = at(&world, me).y;
    let velocity = move_and_slide(&mut world, me, Vec2::new(2.0, 6.0), DT)
        .expect("a character")
        .velocity;
    assert!(velocity.y > 5.0, "{velocity}");
    assert!(at(&world, me).y > start + 0.05, "{}", at(&world, me));
    assert!(!body(&world, me).is_on_floor());
}

#[test]
fn with_stop_on_slope_off_it_still_slides_down_at_rest() {
    let (mut world, me) = on_a_slope(30.0, true);
    if let Some(body) = world.get_mut::<CharacterBody2D>(me) {
        body.stop_on_slope = false;
    }
    let start = at(&world, me);
    let mut velocity = Vec2::ZERO;
    let mut speeds = Vec::new();
    for _ in 0..30 {
        velocity = run(&mut world, me, velocity, None, 1);
        speeds.push(velocity.length());
    }
    let slid = at(&world, me) - start;
    assert!(slid.x < -0.1 && slid.y < 0.0, "down the slope: {slid}");
    assert!(speeds[29] > speeds[10], "gathering speed: {speeds:?}");
}
