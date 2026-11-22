//! The gizmo frame's own tests: grabbing, cancelling, which tool a drag is.

use super::*;
use handle::{AXIS_LENGTH, RING_RADIUS};
use std::f32::consts::FRAC_PI_2;

/// The gizmo's origin in these tests, in screen pixels.
const O: Vec2 = Vec2::new(100.0, 100.0);

fn transform() -> Transform {
    Transform::from_translation(Vec2::ZERO)
}

#[test]
fn cancelling_ends_the_drag() {
    // What Stop needs: `Drag` holds an `Entity` and world-space anchors, and
    // both address a world that a restore is about to despawn and respawn.
    let mut world = voltra_ecs::World::new();
    let entity = world.spawn();
    let mut gizmo = Gizmo {
        drag: Some(Drag::begin(
            entity,
            Tool::Translate,
            Handle::Both,
            Vec2::ZERO,
            transform(),
        )),
        followers: Vec::new(),
    };

    gizmo.cancel_drag();

    assert!(gizmo.drag.is_none());
    assert_eq!(gizmo.active_tool(), None);
}

#[test]
fn each_tool_grabs_its_own_handles() {
    let on_arm = O + Vec2::new(AXIS_LENGTH * 0.5, 0.0);
    let on_ring = O + Vec2::new(RING_RADIUS, 0.0);
    let t = transform();

    assert_eq!(grab_handle(Tool::Translate, on_arm, O, &t), Some(Handle::X));
    assert_eq!(grab_handle(Tool::Scale, on_arm, O, &t), Some(Handle::X));
    assert_eq!(
        grab_handle(Tool::Rotate, on_ring, O, &t),
        Some(Handle::Ring)
    );
}

#[test]
fn a_tool_does_not_grab_another_tools_handle() {
    // The ring passes through empty space for the arm gizmos, and the arms
    // sit inside the ring's middle, which the rotate tool leaves clickable.
    let on_ring = O + Vec2::new(0.0, RING_RADIUS);
    let on_arm = O + Vec2::new(AXIS_LENGTH * 0.5, 0.0);
    let t = transform();

    assert_eq!(grab_handle(Tool::Translate, on_ring, O, &t), None);
    assert_eq!(grab_handle(Tool::Rotate, on_arm, O, &t), None);
}

#[test]
fn the_scale_tool_grabs_the_arms_where_the_rotation_puts_them() {
    // The translate tool's arms stay on the world axes; the scale tool's
    // follow the entity. A quarter turn is where the two disagree.
    let turned = transform().with_rotation(FRAC_PI_2);
    let up = O + Vec2::new(0.0, -AXIS_LENGTH * 0.5);

    assert_eq!(grab_handle(Tool::Scale, up, O, &turned), Some(Handle::X));
    assert_eq!(
        grab_handle(Tool::Translate, up, O, &turned),
        Some(Handle::Y)
    );
}

#[test]
fn a_live_drag_reports_its_own_tool() {
    let mut world = voltra_ecs::World::new();
    let gizmo = Gizmo {
        drag: Some(Drag::begin(
            world.spawn(),
            Tool::Rotate,
            Handle::Ring,
            Vec2::new(1.0, 0.0),
            transform(),
        )),
        followers: Vec::new(),
    };

    assert_eq!(gizmo.active_tool(), Some(Tool::Rotate));
}
