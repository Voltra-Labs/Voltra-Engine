//! Which camera is active, and the view it describes.

use super::*;
use crate::hierarchy::link::set_parent;
use crate::scene_id::SceneId;
use crate::transform::Transform;

fn spawn(world: &mut World, camera: Camera, at: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Transform::from_translation(at));
    world.insert(entity, camera);
    entity
}

#[test]
fn a_scene_without_a_camera_has_no_active_one() {
    let mut world = World::new();
    world.spawn();
    assert_eq!(active(&world), None);
}

#[test]
fn the_only_camera_is_the_active_one() {
    let mut world = World::new();
    let camera = spawn(&mut world, Camera::default(), Vec2::ZERO);
    assert_eq!(active(&world), Some(camera));
}

#[test]
fn the_highest_priority_wins() {
    let mut world = World::new();
    spawn(&mut world, Camera::default(), Vec2::ZERO);
    let winner = spawn(
        &mut world,
        Camera::default().with_priority(5),
        Vec2::new(3.0, 0.0),
    );
    spawn(&mut world, Camera::default().with_priority(-1), Vec2::ZERO);

    assert_eq!(active(&world), Some(winner));
}

#[test]
fn an_inactive_camera_is_never_chosen() {
    let mut world = World::new();
    let low = spawn(&mut world, Camera::default(), Vec2::ZERO);
    spawn(
        &mut world,
        Camera {
            active: false,
            ..Camera::default().with_priority(100)
        },
        Vec2::ZERO,
    );

    assert_eq!(active(&world), Some(low));
}

#[test]
fn a_tie_breaks_on_the_lower_entity_index() {
    let mut world = World::new();
    let first = spawn(&mut world, Camera::default(), Vec2::ZERO);
    let second = spawn(&mut world, Camera::default(), Vec2::ZERO);

    assert!(first.index() < second.index());
    assert_eq!(
        active(&world),
        Some(first),
        "ties must not depend on query order"
    );
}

#[test]
fn the_view_sits_where_the_transform_says() {
    let mut world = World::new();
    let entity = spawn(&mut world, Camera::default(), Vec2::new(3.0, -4.0));

    let view = view(&world, entity, 1.5).expect("it has a camera");
    assert_eq!(view.position, Vec2::new(3.0, -4.0));
    assert_eq!(view.aspect, 1.5);
}

#[test]
fn a_size_is_half_the_world_height_the_view_covers() {
    let mut world = World::new();
    let entity = spawn(&mut world, Camera::new(5.0), Vec2::ZERO);

    let view = view(&world, entity, 2.0).expect("it has a camera");
    assert!((view.half_extents().y - 5.0).abs() < 1e-5);
    assert!(
        (view.half_extents().x - 10.0).abs() < 1e-5,
        "aspect widens x"
    );
}

#[test]
fn a_parented_camera_is_where_its_parent_put_it() {
    let mut world = World::new();
    let rig = spawn(&mut world, Camera::default(), Vec2::new(10.0, 0.0));
    let entity = spawn(&mut world, Camera::default(), Vec2::new(1.0, 2.0));
    set_parent(&mut world, entity, rig).expect("a plain reparent");

    let view = view(&world, entity, 1.0).expect("it has a camera");
    assert!((view.position - Vec2::new(11.0, 2.0)).length() < 1e-5);
}

#[test]
fn a_camera_without_a_transform_sits_at_the_origin() {
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, Camera::default());

    let view = view(&world, entity, 1.0).expect("it has a camera");
    assert_eq!(view.position, Vec2::ZERO);
}

#[test]
fn an_entity_without_a_camera_has_no_view() {
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, Transform::default());
    assert!(view(&world, entity, 1.0).is_none());
}

#[test]
fn a_degenerate_size_still_produces_a_finite_view() {
    // A size dragged to zero, or typed as one into the inspector. The
    // clamp lives in `Camera2D::set_zoom`; this is the test that the path
    // through this type actually reaches it.
    let mut world = World::new();
    let entity = spawn(&mut world, Camera::new(0.0), Vec2::ZERO);

    let view = view(&world, entity, 1.0).expect("it has a camera");
    assert_eq!(view.zoom(), Camera2D::MAX_ZOOM);
    assert!(view.half_extents().is_finite());
}

#[test]
fn a_nan_size_leaves_a_usable_view() {
    let mut world = World::new();
    let entity = spawn(&mut world, Camera::new(f32::NAN), Vec2::ZERO);

    let view = view(&world, entity, 1.0).expect("it has a camera");
    assert!(
        view.half_extents().is_finite(),
        "a NaN must not poison the frame"
    );
}

#[test]
fn the_size_bounds_are_the_zoom_bounds_the_other_way_up() {
    assert!((Camera::new(Camera::MAX_SIZE).zoom() - Camera2D::MIN_ZOOM).abs() < 1e-9);
    assert!((Camera::new(Camera::MIN_SIZE).zoom() - Camera2D::MAX_ZOOM).abs() < 1e-3);
}

#[test]
fn the_active_view_frames_the_camera_that_wins() {
    let mut world = World::new();
    spawn(&mut world, Camera::new(2.0), Vec2::new(-5.0, 0.0));
    spawn(
        &mut world,
        Camera::new(4.0).with_priority(1),
        Vec2::new(7.0, -3.0),
    );

    let view = active_view(&world, 1.5).expect("one of them is active");
    assert_eq!(view.position, Vec2::new(7.0, -3.0));
    assert_eq!(view.zoom(), 0.25);
    assert_eq!(view.aspect, 1.5);
}

#[test]
fn a_scene_without_an_active_camera_has_no_active_view() {
    let mut world = World::new();
    spawn(
        &mut world,
        Camera {
            active: false,
            ..Default::default()
        },
        Vec2::ZERO,
    );

    assert!(active_view(&world, 1.0).is_none());
}
