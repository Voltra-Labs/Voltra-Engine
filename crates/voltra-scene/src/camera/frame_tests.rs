//! The frame's order over both dimensions: priority, 3D first at a tie,
//! pruning within a dimension, overlays, producers before consumers.

use voltra_assets::{AssetPath, Handle};
use voltra_ecs::{Entity, World};
use voltra_render::glam::{Vec2, Vec3};

use super::*;
use crate::{Camera3D, CameraClear, SceneId, Sprite2D, Transform2D, Transform3D, UiRect};

fn camera_2d(world: &mut World, camera: Camera2D) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, Transform2D::default());
    world.insert(e, camera);
    e
}

fn camera_3d(world: &mut World, camera: Camera3D) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, Transform3D::from_translation(Vec3::Z * 5.0));
    world.insert(e, camera);
    e
}

/// Points `camera` at a render texture resolved to `handle`.
fn into_texture(world: &mut World, camera: Entity, handle: Handle<voltra_render::Texture>) {
    let mut target = CameraTarget::new(AssetPath::new("t.rendertexture.ron").expect("a path"));
    target.handle = Some(handle);
    world.insert(camera, target);
}

/// A world sprite showing the texture `handle`.
fn showing(world: &mut World, handle: Handle<voltra_render::Texture>) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, Transform2D::from_translation(Vec2::ZERO));
    world.insert(
        e,
        Sprite2D {
            texture_handle: Some(handle),
            ..Sprite2D::default()
        },
    );
    e
}

fn order(frame: &FrameShots) -> Vec<(Entity, bool)> {
    frame
        .shots
        .iter()
        .map(|s| (s.shot.entity(), s.overlay))
        .collect()
}

#[test]
fn at_equal_priority_3d_draws_first_and_2d_over_it_is_an_overlay() {
    let mut world = World::new();
    // Spawned 2D first, so only the tie-break puts 3D first.
    let flat = camera_2d(&mut world, Camera2D::default());
    let deep = camera_3d(&mut world, Camera3D::default());
    let frame = frame_shots(&world);
    assert_eq!(order(&frame), vec![(deep, false), (flat, true)]);
    assert!(frame.has_3d());
    assert!(frame.cycle.is_empty());
}

#[test]
fn priority_is_the_order_so_a_3d_picture_in_a_picture_draws_over_2d() {
    let mut world = World::new();
    let game = camera_2d(&mut world, Camera2D::default());
    let preview = camera_3d(
        &mut world,
        Camera3D {
            viewport: Viewport {
                x: 0.6,
                y: 0.1,
                width: 0.3,
                height: 0.3,
            },
            ..Camera3D::default().with_priority(1)
        },
    );
    let frame = frame_shots(&world);
    // The 2D game first; the 3D preview over it, see-through around its model.
    assert_eq!(order(&frame), vec![(game, false), (preview, true)]);
    // A colour of its own makes it opaque.
    world.get_mut::<Camera3D>(preview).expect("spawned").clear =
        CameraClear::Color([0.0, 0.0, 0.0, 1.0]);
    assert_eq!(
        order(&frame_shots(&world)),
        vec![(game, false), (preview, false)]
    );
}

#[test]
fn pruning_holds_within_a_dimension_only() {
    let mut world = World::new();
    // Two full-screen 2D cameras: only the higher draws, as in 0.4.
    let low = camera_2d(&mut world, Camera2D::default());
    let high = camera_2d(&mut world, Camera2D::default().with_priority(2));
    // A 3D camera between them is never hidden by either.
    let deep = camera_3d(&mut world, Camera3D::default().with_priority(1));
    let frame = frame_shots(&world);
    assert_eq!(order(&frame), vec![(deep, false), (high, true)]);
    assert!(!order(&frame).iter().any(|(e, _)| *e == low));
    // And two full-screen 3D cameras: the higher, over the lower.
    let higher = camera_3d(&mut world, Camera3D::default().with_priority(3));
    let frame = frame_shots(&world);
    assert_eq!(order(&frame), vec![(high, false), (higher, true)]);
}

#[test]
fn a_split_screen_of_3d_left_and_2d_right_has_no_overlay() {
    let mut world = World::new();
    let left = camera_3d(
        &mut world,
        Camera3D {
            viewport: Viewport::LEFT,
            ..Camera3D::default()
        },
    );
    let right = camera_2d(
        &mut world,
        Camera2D {
            viewport: Viewport::RIGHT,
            ..Camera2D::default()
        },
    );
    assert_eq!(
        order(&frame_shots(&world)),
        vec![(left, false), (right, false)]
    );
}

#[test]
fn a_texture_shot_draws_after_the_shot_filling_a_texture_it_shows() {
    let mut world = World::new();
    let minimap = Handle::forge(7, 0);
    let monitor = Handle::forge(8, 0);
    // A 2D camera into `monitor`, low priority, whose world shows the 3D
    // minimap; the 3D camera into `minimap` has the higher priority.
    let flat = camera_2d(&mut world, Camera2D::default());
    into_texture(&mut world, flat, monitor);
    let deep = camera_3d(&mut world, Camera3D::default().with_priority(5));
    into_texture(&mut world, deep, minimap);
    showing(&mut world, minimap);
    let frame = frame_shots(&world);
    assert_eq!(order(&frame), vec![(deep, false), (flat, false)]);
    assert!(frame.cycle.is_empty());
    assert_eq!(frame.screen().count(), 0);
    // A HUD image is drawn after every shot: it orders nothing.
    let mut world = World::new();
    let flat = camera_2d(&mut world, Camera2D::default());
    into_texture(&mut world, flat, monitor);
    let other = camera_2d(&mut world, Camera2D::default().with_priority(1));
    into_texture(&mut world, other, minimap);
    let hud = showing(&mut world, monitor);
    world.insert(hud, UiRect::default());
    assert_eq!(
        order(&frame_shots(&world)),
        vec![(flat, false), (other, false)]
    );
}

#[test]
fn two_texture_shots_showing_each_other_are_a_cycle() {
    let mut world = World::new();
    let (a_tex, b_tex) = (Handle::forge(1, 0), Handle::forge(2, 0));
    let a = camera_2d(&mut world, Camera2D::default());
    into_texture(&mut world, a, a_tex);
    let b = camera_2d(&mut world, Camera2D::default().with_priority(1));
    into_texture(&mut world, b, b_tex);
    showing(&mut world, a_tex);
    showing(&mut world, b_tex);
    let frame = frame_shots(&world);
    // Each 2D camera shows the other's texture, not its own: the first by
    // priority draws first, with the other's last frame.
    assert_eq!(order(&frame), vec![(a, false), (b, false)]);
    assert_eq!(frame.cycle, vec![a]);
}

#[test]
fn an_unresolved_or_inactive_3d_camera_is_left_out() {
    let mut world = World::new();
    let unresolved = camera_3d(&mut world, Camera3D::default());
    world.insert(
        unresolved,
        CameraTarget::new(AssetPath::new("m.rendertexture.ron").expect("a path")),
    );
    camera_3d(
        &mut world,
        Camera3D {
            active: false,
            ..Camera3D::default()
        },
    );
    let frame = frame_shots(&world);
    assert!(frame.shots.is_empty());
    assert!(!frame.has_3d());
}
