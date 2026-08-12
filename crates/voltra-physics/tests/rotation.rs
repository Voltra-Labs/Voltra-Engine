//! What rotation does to a scene: a box that must not rock, one that tips onto
//! a face, a plank about its fulcrum, and both slopes.

mod common;

use common::*;
use voltra_ecs::{Entity, World};
use voltra_physics::ImpulseCache;
use voltra_render::glam::Vec2;
use voltra_scene::RigidBody;

#[test]
fn a_box_dropped_flat_settles_without_rocking() {
    // The reason a manifold carries two points. Given one, the solver
    // corrects a single corner per step, the box tips the other way, and it
    // rocks for as long as the scene runs.
    let mut world = World::new();
    spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(10.0, 1.0), None);
    let crate_ = spawn_box(&mut world, Vec2::new(0.0, 0.6), Vec2::splat(0.5), Some(1.0));

    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 300);

    let angle = rotation(&world, crate_);
    assert!(angle.abs() < 0.01, "it rocked to {angle} rad");
    assert!(
        spin(&world, crate_).abs() < 0.05,
        "{}",
        spin(&world, crate_)
    );
    let y = translation(&world, crate_).y;
    assert!((y - 0.5).abs() < 0.05, "it should rest at y = 0.5, got {y}");
}

#[test]
fn a_box_dropped_on_a_corner_tips_onto_a_face() {
    let mut world = World::new();
    spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(10.0, 1.0), None);
    let crate_ = spawn_turned_box(
        &mut world,
        Vec2::new(0.0, 1.2),
        Vec2::splat(0.5),
        Some(1.0),
        0.5,
    );

    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 600);

    // Tipped back flat: less than half a turn of a quarter turn away from
    // where a face lies on the floor.
    let angle = rotation(&world, crate_);
    assert!(angle.abs() < 0.15, "it stayed on its corner at {angle} rad");
    assert!(spin(&world, crate_).abs() < 0.1, "still rocking");
}

#[test]
fn a_loaded_plank_rotates_about_its_fulcrum() {
    // A plank across a narrow support, weighted on its right end. Nothing
    // about this scene resolves without torque: with 11b-2's solver the
    // plank stays level and the weight sits in the air.
    let mut world = World::new();
    spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(0.2, 1.0), None);
    let plank = spawn_box(
        &mut world,
        Vec2::new(0.0, 0.1),
        Vec2::new(2.0, 0.1),
        Some(1.0),
    );
    spawn_box(&mut world, Vec2::new(1.5, 0.6), Vec2::splat(0.4), Some(5.0));

    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 300);

    // Clockwise: the loaded end is the right one, and y is up.
    assert!(
        rotation(&world, plank) < -0.1,
        "the loaded end must go down, got {}",
        rotation(&world, plank)
    );
}

#[test]
fn an_off_centre_hit_spins_a_body_the_right_way() {
    // A box dropped onto a peg under its left corner. The push is upward
    // and to the left of the centre, so the torque turns it clockwise.
    let mut world = World::new();
    spawn_box(&mut world, Vec2::new(-0.45, -1.0), Vec2::splat(0.1), None);
    let crate_ = spawn_box(&mut world, Vec2::new(0.0, 0.0), Vec2::splat(0.5), Some(1.0));

    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 30);

    assert!(
        spin(&world, crate_) < -0.05,
        "a hit left of centre turns a body clockwise, got {}",
        spin(&world, crate_)
    );
    assert!(
        velocity(&world, crate_).y > -100.0,
        "and it is still finite"
    );
}

#[test]
fn a_locked_body_takes_the_hit_without_turning() {
    let mut world = World::new();
    spawn_box(&mut world, Vec2::new(-0.45, -1.0), Vec2::splat(0.1), None);
    let crate_ = spawn_box(&mut world, Vec2::new(0.0, 0.0), Vec2::splat(0.5), Some(1.0));
    world
        .get_mut::<RigidBody>(crate_)
        .expect("the crate")
        .lock_rotation = true;

    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 30);

    assert_eq!(spin(&world, crate_), 0.0);
    assert_eq!(rotation(&world, crate_), 0.0);
    // And the linear half of the contact still happened.
    assert!(
        velocity(&world, crate_).y > -5.0,
        "the peg must still hold it up: {:?}",
        velocity(&world, crate_)
    );
}

#[test]
fn a_box_on_a_rough_slope_does_not_slide() {
    // tan(0.3) ≈ 0.31, well under a friction of one.
    let (mut world, crate_) = ramp(0.3, 1.0);
    let start = translation(&world, crate_);

    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 600);

    let moved = translation(&world, crate_) - start;
    assert!(moved.length() < 0.1, "it slid {moved:?}");
}

#[test]
fn a_box_on_a_frictionless_slope_slides_without_spinning() {
    let (mut world, crate_) = ramp(0.3, 0.0);
    let start = translation(&world, crate_);

    let mut cache = ImpulseCache::default();
    // One second. Longer and it runs off the end of the ramp, which says
    // nothing about friction.
    run(&mut world, &mut cache, G, 60);

    // The ramp is turned counter-clockwise, so downhill is -x.
    let moved = translation(&world, crate_) - start;
    assert!(
        moved.x < -0.5 && moved.y < 0.0,
        "it should slide down the slope, got {moved:?}"
    );
    // A box on a plane has no reason to turn without friction, and one that
    // does is a sign the two contact points are being pushed unequally.
    assert!(
        spin(&world, crate_).abs() < 0.2,
        "it spun at {}",
        spin(&world, crate_)
    );
}

#[test]
fn a_stack_of_rotated_boxes_does_not_gain_energy() {
    let mut world = World::new();
    spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(10.0, 1.0), None);
    let boxes: Vec<Entity> = (0..4)
        .map(|i| {
            spawn_turned_box(
                &mut world,
                Vec2::new(0.0, 0.55 + i as f32 * 1.05),
                Vec2::splat(0.5),
                Some(1.0),
                0.05 * (i as f32 - 1.5),
            )
        })
        .collect();

    let energy = |world: &World| -> f32 {
        boxes
            .iter()
            .map(|entity| {
                let body = world.get::<RigidBody>(*entity).expect("a body");
                body.velocity.length_squared() + body.angular_velocity * body.angular_velocity
            })
            .sum()
    };

    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 10);
    let early = energy(&world);
    run(&mut world, &mut cache, G, 300);

    assert!(
        energy(&world) <= early + 1e-3,
        "energy grew from {early} to {}",
        energy(&world)
    );
}

#[test]
fn a_body_spun_absurdly_fast_stays_in_the_world() {
    let mut world = World::new();
    spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(10.0, 1.0), None);
    let crate_ = spawn_box(&mut world, Vec2::new(0.0, 0.6), Vec2::splat(0.5), Some(1.0));
    world
        .get_mut::<RigidBody>(crate_)
        .expect("the crate")
        .angular_velocity = 1.0e6;

    let mut cache = ImpulseCache::default();
    // Half a second: a box spinning at the cap rubs against the floor and
    // is fired sideways, which is what friction on a wheel does. Run it
    // longer and it leaves the end of the floor, which is not the point.
    run(&mut world, &mut cache, G, 30);

    let position = translation(&world, crate_);
    assert!(position.is_finite(), "{position:?}");
    assert!(rotation(&world, crate_).is_finite());
    assert!(
        position.y > 0.0 && position.x.abs() < 9.0,
        "it must stay on top of the floor, not through it: {position:?}"
    );
}
