//! A shake moves and turns the view, never the transform: whatever writes
//! the transform — absolutely or by adding to it — the camera rests where
//! that writer put it.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::tests::{placed, DT};
use super::*;
use crate::camera::eye;
use crate::{Camera, Transform};

fn shaking_camera(world: &mut World, at: Vec2, rotation: f32) -> Entity {
    let camera = placed(world, at);
    world.get_mut::<Transform>(camera).expect("t").rotation = rotation;
    world.insert(camera, Camera::new(3.0));
    world.insert(
        camera,
        CameraShake {
            max_angle: 0.3,
            max_offset: Vec2::splat(0.4),
            ..CameraShake::default()
        },
    );
    world
        .get_mut::<CameraShake>(camera)
        .expect("s")
        .add_trauma(1.0);
    camera
}

fn transform(world: &mut World, camera: Entity) -> &mut Transform {
    world.get_mut::<Transform>(camera).expect("t")
}

/// Runs until the trauma is spent, and a frame more so the view rests.
fn run_out(world: &mut World, each: impl Fn(&mut World)) {
    for _ in 0..200 {
        each(world);
        update(world, DT, 1.0, None);
    }
}

#[test]
fn a_turning_shake_moves_the_view_and_fades_to_rest() {
    let mut world = World::new();
    let camera = shaking_camera(&mut world, Vec2::new(1.0, 2.0), 0.2);
    let mut swung = 0.0f32;
    let mut moved = 0.0f32;
    for _ in 0..120 {
        update(&mut world, DT, 1.0, None);
        let (position, turn) = eye(&world, camera);
        assert!((turn - 0.2).abs() <= 0.3 + 1e-5, "within the most it turns");
        swung = swung.max((turn - 0.2).abs());
        moved = moved.max(position.distance(Vec2::new(1.0, 2.0)));
        // The transform is the rest, every frame.
        assert_eq!(transform(&mut world, camera).rotation, 0.2);
    }
    assert!(swung > 0.01 && moved > 0.01, "it shook: {swung} {moved}");
    run_out(&mut world, |_| {});
    let view = crate::camera::view(&world, camera, 1.0).expect("a camera");
    assert_eq!(view.rotation, 0.2);
    assert_eq!(view.position, Vec2::new(1.0, 2.0));
}

#[test]
fn an_absolute_writer_under_a_shake_rests_exactly_where_it_wrote() {
    // A tween or a clip setting the rotation and position every frame: the
    // shake must not leave anything behind once it is spent.
    let mut world = World::new();
    let camera = shaking_camera(&mut world, Vec2::ZERO, 0.0);
    run_out(&mut world, |world| {
        let t = transform(world, camera);
        t.rotation = 0.5;
        t.translation = Vec2::new(3.0, -1.0);
    });
    let (position, turn) = eye(&world, camera);
    assert_eq!(turn, 0.5);
    assert_eq!(position, Vec2::new(3.0, -1.0));
}

#[test]
fn a_relative_writer_under_a_shake_keeps_all_it_added() {
    // A game turning and moving the camera a little every frame: what it
    // added is all there once the shake is spent.
    let mut world = World::new();
    let camera = shaking_camera(&mut world, Vec2::ZERO, 0.0);
    let mut frames = 0u32;
    for _ in 0..200 {
        let t = transform(&mut world, camera);
        t.rotation += 0.001;
        t.translation += Vec2::new(0.01, 0.0);
        frames += 1;
        update(&mut world, DT, 1.0, None);
    }
    let (position, turn) = eye(&world, camera);
    assert!((turn - 0.001 * frames as f32).abs() < 1e-4, "{turn}");
    assert!(
        (position.x - 0.01 * frames as f32).abs() < 1e-4,
        "{position}"
    );
    assert!(position.y.abs() < 1e-6);
}

#[test]
fn the_offset_runs_along_the_turned_view() {
    let mut world = World::new();
    let camera = shaking_camera(&mut world, Vec2::ZERO, std::f32::consts::FRAC_PI_2);
    world.get_mut::<CameraShake>(camera).expect("s").max_angle = 0.0;
    update(&mut world, DT, 1.0, None);
    let shake = world.get::<CameraShake>(camera).expect("s");
    let applied = shake.applied_offset();
    let (position, _) = eye(&world, camera);
    // A quarter turn left: the view's right is the world's up.
    assert!(position.abs_diff_eq(Vec2::new(-applied.y, applied.x), 1e-6));
}

#[test]
fn a_shake_without_a_max_angle_never_turns_and_is_not_written() {
    let mut world = World::new();
    let camera = placed(&mut world, Vec2::ZERO);
    world.insert(camera, Camera::default());
    world.insert(camera, CameraShake::default());
    world
        .get_mut::<CameraShake>(camera)
        .expect("s")
        .add_trauma(1.0);
    for _ in 0..30 {
        update(&mut world, DT, 1.0, None);
        assert_eq!(eye(&world, camera).1, 0.0);
    }
    let written = ron::to_string(&CameraShake::default()).expect("writes");
    assert!(!written.contains("max_angle"), "{written}");
    let read: CameraShake =
        ron::from_str("(max_offset: (0.3, 0.3), frequency: 15.0, decay: 1.2)").expect("reads");
    assert_eq!(read.max_angle, 0.0);
}
