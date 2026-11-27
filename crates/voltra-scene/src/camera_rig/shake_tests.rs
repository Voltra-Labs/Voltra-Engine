//! A shake that turns: rotational trauma fading to rest.

use voltra_ecs::World;
use voltra_render::glam::Vec2;

use super::tests::{at, placed, DT};
use super::*;
use crate::{Camera, Transform};

fn turn_of(world: &World, entity: voltra_ecs::Entity) -> f32 {
    world.get::<Transform>(entity).expect("placed").rotation
}

#[test]
fn a_turning_shake_fades_back_to_the_rest_angle_and_never_drifts() {
    let mut world = World::new();
    let camera = placed(&mut world, Vec2::new(1.0, 2.0));
    world.get_mut::<Transform>(camera).expect("t").rotation = 0.2;
    world.insert(camera, Camera::new(3.0));
    world.insert(
        camera,
        CameraShake {
            max_angle: 0.1,
            ..CameraShake::default()
        },
    );
    let mut swung = 0.0f32;
    // Three knocks, each left to fade: the rest angle must survive all.
    for _ in 0..3 {
        world
            .get_mut::<CameraShake>(camera)
            .expect("s")
            .add_trauma(1.0);
        for _ in 0..120 {
            update(&mut world, DT, 1.0);
            let shake = world.get::<CameraShake>(camera).expect("s");
            let off = turn_of(&world, camera) - 0.2;
            assert!((off - shake.applied_angle()).abs() < 1e-5);
            assert!(off.abs() <= 0.1 + 1e-5, "within the most it turns: {off}");
            swung = swung.max(off.abs());
        }
        assert_eq!(world.get::<CameraShake>(camera).expect("s").trauma, 0.0);
        assert!(
            (turn_of(&world, camera) - 0.2).abs() < 1e-6,
            "back at rest: {}",
            turn_of(&world, camera)
        );
        assert!((at(&world, camera) - Vec2::new(1.0, 2.0)).length() < 1e-5);
    }
    assert!(swung > 0.01, "it turned: {swung}");
    // And the view turns with it.
    let view = crate::camera::view(&world, camera, 1.0).expect("a camera");
    assert!((view.rotation - 0.2).abs() < 1e-6);
}

#[test]
fn a_shake_without_a_max_angle_never_turns_and_is_not_written() {
    let mut world = World::new();
    let camera = placed(&mut world, Vec2::ZERO);
    world.insert(camera, CameraShake::default());
    world
        .get_mut::<CameraShake>(camera)
        .expect("s")
        .add_trauma(1.0);
    for _ in 0..30 {
        update(&mut world, DT, 1.0);
        assert_eq!(turn_of(&world, camera), 0.0);
    }
    let written = ron::to_string(&CameraShake::default()).expect("writes");
    assert!(!written.contains("max_angle"), "{written}");
    let read: CameraShake =
        ron::from_str("(max_offset: (0.3, 0.3), frequency: 15.0, decay: 1.2)").expect("reads");
    assert_eq!(read.max_angle, 0.0);
}

#[test]
fn a_turn_the_game_makes_while_shaking_is_kept() {
    // The game turns the camera between frames: settling takes off only
    // what the shake added, so the game's turn stays.
    let mut world = World::new();
    let camera = placed(&mut world, Vec2::ZERO);
    world.insert(
        camera,
        CameraShake {
            max_angle: 0.3,
            ..CameraShake::default()
        },
    );
    world
        .get_mut::<CameraShake>(camera)
        .expect("s")
        .add_trauma(1.0);
    for _ in 0..10 {
        update(&mut world, DT, 1.0);
    }
    world.get_mut::<Transform>(camera).expect("t").rotation += 1.0;
    for _ in 0..200 {
        update(&mut world, DT, 1.0);
    }
    assert!((turn_of(&world, camera) - 1.0).abs() < 1e-5);
}
