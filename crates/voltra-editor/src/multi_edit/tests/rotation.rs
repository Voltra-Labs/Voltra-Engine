//! A rotation is one value across a selection: its four components are a
//! quaternion, and writing the ones that changed into another entity's
//! mixes two rotations into something that is neither.

use voltra_ecs::{Entity, World};
use voltra_render::glam::{Quat, Vec3};
use voltra_scene::format::serial_tree::Step;
use voltra_scene::format::ComponentRegistry;
use voltra_scene::{SceneId, Transform3D};

use super::super::Touch;
use super::begin;

fn turned(world: &mut World, rotation: Quat) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Transform3D::default().with_rotation(rotation));
    entity
}

#[test]
fn a_rotation_typed_on_the_primary_is_written_whole() {
    let registry = ComponentRegistry::with_defaults();
    let mut world = World::new();
    let other = turned(&mut world, Quat::from_rotation_x(0.8));
    let primary = turned(&mut world, Quat::from_rotation_y(0.3));
    let mut edit = begin(&world, &registry, &[other, primary]);

    // What the inspector does for a typed yaw of 90°: write the new
    // quaternion, and touch the rotation as a whole.
    let typed = Transform3D::from_euler_degrees(Vec3::new(0.0, 90.0, 0.0)).rotation;
    world.get_mut::<Transform3D>(primary).expect("3D").rotation = typed;
    edit.touch(
        Transform3D::NAME,
        vec![Step::Field("rotation")],
        Touch::Value,
    );
    edit.finish(&mut world, &registry);

    let theirs = world.get::<Transform3D>(other).expect("3D").rotation;
    assert!(theirs.abs_diff_eq(typed, 1e-7), "{theirs:?} vs {typed:?}");
    assert!((theirs.length() - 1.0).abs() < 1e-6);
}

#[test]
fn without_the_touch_only_the_changed_components_would_go_across() {
    // Pins why the inspector touches: the field-by-field write alone keeps
    // the other entity's x and gives it the primary's y and w — not a
    // rotation either of them had (the load's repair makes it unit length,
    // which does not make it right).
    let registry = ComponentRegistry::with_defaults();
    let mut world = World::new();
    let other = turned(&mut world, Quat::from_rotation_x(0.8));
    let primary = turned(&mut world, Quat::from_rotation_y(0.3));
    let edit = begin(&world, &registry, &[other, primary]);
    let typed = Transform3D::from_euler_degrees(Vec3::new(0.0, 90.0, 0.0)).rotation;
    world.get_mut::<Transform3D>(primary).expect("3D").rotation = typed;
    edit.finish(&mut world, &registry);

    let theirs = world.get::<Transform3D>(other).expect("3D").rotation;
    assert!(!theirs.abs_diff_eq(typed, 1e-3), "{theirs:?}");
    assert!(
        theirs.x.abs() > 0.1,
        "the other's own x was kept: {theirs:?}"
    );
}
