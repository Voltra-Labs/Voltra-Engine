//! Joints on heavy bodies hold as they do on light ones. A joint's effective
//! mass goes as the inverse mass squared, so a fixed threshold for "nothing
//! can move" would take ten tonnes for an immovable body and drop the joint.

mod common;

use common::*;
use voltra_ecs::{Entity, World};
use voltra_physics::ImpulseCache;
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, Joint2D, JointKind, RigidBody, Transform};

/// Ten tonnes.
const HEAVY: f32 = 10_000.0;

/// A ten-tonne ball of radius 0.2 at the origin, `kind` on it at its centre,
/// held to the world at `anchor_b`.
fn heavy_ball(world: &mut World, anchor_b: Vec2, kind: JointKind) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform::default());
    world.insert(entity, Collider::Circle { radius: 0.2 });
    world.insert(entity, RigidBody::new_dynamic(HEAVY));
    world.insert(
        entity,
        Joint2D {
            anchor_b,
            kind,
            ..Joint2D::default()
        },
    );
    entity
}

#[test]
fn a_mouse_joint_pulls_ten_tonnes_to_its_target() {
    let mut world = World::new();
    let target = Vec2::new(2.0, 1.0);
    let body = heavy_ball(
        &mut world,
        target,
        JointKind::Mouse {
            max_force: 1000.0 * HEAVY,
            hertz: 5.0,
            damping_ratio: 0.7,
        },
    );
    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 120);
    let at = translation(&world, body);
    assert!((at - target).length() < 0.05, "pulled to {target}: {at}");
}

#[test]
fn a_pin_holds_ten_tonnes_through_its_centre() {
    let mut world = World::new();
    let body = heavy_ball(&mut world, Vec2::ZERO, JointKind::default());
    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 120);
    let at = translation(&world, body);
    assert!(at.length() < 0.01, "held on its pin: {at}");
}

#[test]
fn a_ten_tonne_slider_stays_on_its_line() {
    let mut world = World::new();
    // A crate a unit on each side: heavy enough to turn slowly too.
    let body = spawn_box(&mut world, Vec2::ZERO, Vec2::splat(0.5), Some(HEAVY));
    world.insert(
        body,
        Joint2D {
            kind: JointKind::prismatic(Vec2::X),
            ..Joint2D::default()
        },
    );
    world.get_mut::<RigidBody>(body).expect("its body").velocity = Vec2::new(1.0, 0.0);
    let mut cache = ImpulseCache::default();
    for _ in 0..120 {
        run(&mut world, &mut cache, G, 1);
        let at = translation(&world, body);
        assert!(at.y.abs() < 0.01, "held on its line under gravity: {at}");
        assert!(rotation(&world, body).abs() < 0.01, "and its angle");
    }
    let x = translation(&world, body).x;
    assert!((x - 2.0).abs() < 0.05, "sliding freely along it: {x}");
}
