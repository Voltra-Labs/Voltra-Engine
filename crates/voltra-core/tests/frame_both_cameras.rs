//! One frame of both worlds, screenshotted headless, as cameras: a 3D
//! picture in a picture over a 2D game, two 2D cameras beside a 3D one, a
//! 3D camera into a render texture shown on a sprite and in the HUD, and a
//! split screen of the two. Each skips without a GPU adapter.
//!
//! Set `VOLTRA_SHOT_DIR` to a folder to keep the pictures.

mod both;

use both::*;

use voltra_assets::AssetPath;
use voltra_core::App;
use voltra_render::glam::{Vec2, Vec3};
use voltra_scene::{
    Camera2D, Camera3D, CameraClear, CameraTarget, SceneId, Sprite2D, UiRect, Viewport,
};
use voltra_testkit::scratch_root;

#[test]
fn a_3d_picture_in_a_picture_at_a_higher_priority_draws_over_a_2d_game() {
    // Top right, a quarter of each side.
    let corner = Viewport {
        x: 0.75,
        y: 0.0,
        width: 0.25,
        height: 0.25,
    };
    for (opaque, name) in [(true, "both-pip-opaque"), (false, "both-pip-overlay")] {
        let mut app = App::default();
        let world = &mut app.world;
        // The 2D game: a green field filling its full-screen camera.
        camera_2d(world, Vec2::ZERO, Camera2D::new(5.0));
        sprite(world, Vec2::ZERO, 40.0, rgba(GREEN));
        let preview = camera_3d(
            world,
            Camera3D {
                viewport: corner,
                ..Camera3D::default().with_priority(1)
            },
        );
        if !opaque {
            world.get_mut::<Camera3D>(preview).expect("spawned").clear = CameraClear::Background;
        }
        cube(world, Vec3::ZERO);
        let Some((shot, _)) = shoot(app, name) else {
            return;
        };
        // The corner: 80 × 45 pixels from x 240. Its centre is the cube.
        let (px, py) = (240 + 40, 22);
        assert!(
            near(at(&shot, px, py), CUBE, 2),
            "{name}: {:?}",
            at(&shot, px, py)
        );
        let around = at(&shot, 243, 2);
        if opaque {
            assert!(near(around, SKY, 2), "{name}: {around:?}");
        } else {
            assert!(
                near(around, GREEN, 2),
                "{name}: the game shows through: {around:?}"
            );
        }
        // The rest of the screen is the game.
        assert!(near(at(&shot, 100, 100), GREEN, 2), "{name}");
        assert_eq!(count(&shot, 0..236, 0..HEIGHT, CUBE), 0, "{name}");
    }
}

#[test]
fn two_full_screen_2d_cameras_draw_only_the_highest_beside_a_3d_one() {
    let mut app = App::default();
    let world = &mut app.world;
    camera_3d(world, Camera3D::default());
    cube(world, Vec3::ZERO);
    // A blue sprite far left, seen only by the low camera; the high one
    // looks far right at nothing.
    sprite(world, Vec2::new(-50.0, 0.0), 2.0, rgba(BLUE));
    camera_2d(world, Vec2::new(-50.0, 0.0), Camera2D::new(5.0));
    camera_2d(
        world,
        Vec2::new(50.0, 0.0),
        Camera2D::new(5.0).with_priority(1),
    );
    let Some((shot, _)) = shoot(app, "both-two-2d") else {
        return;
    };
    assert_eq!(
        count(&shot, 0..WIDTH, 0..HEIGHT, BLUE),
        0,
        "the low 2D camera"
    );
    assert!(
        near(at(&shot, WIDTH / 2, HEIGHT / 2), CUBE, 2),
        "the 3D camera still draws"
    );
}

#[test]
fn a_3d_camera_into_a_render_texture_shows_on_a_sprite_and_in_the_hud() {
    let root = scratch_root();
    std::fs::write(
        root.join("map.rendertexture.ron"),
        "(version: 1, width: 64, height: 64, nearest: true)",
    )
    .expect("the render texture");
    let texture = || AssetPath::new("map.rendertexture.ron").expect("path");
    let mut app = App::default().with_asset_root(&root);
    let world = &mut app.world;
    let eye = camera_3d(world, Camera3D::default());
    world.insert(eye, CameraTarget::new(texture()));
    cube(world, Vec3::ZERO);
    // The 2D game shows the texture on a sprite left of centre, 6 units
    // (108 px) square, and the HUD in the top-right corner, 64 px.
    camera_2d(world, Vec2::ZERO, Camera2D::new(5.0));
    let monitor = sprite(world, Vec2::new(-4.0, 0.0), 6.0, [1.0; 4]);
    world.get_mut::<Sprite2D>(monitor).expect("spawned").texture = Some(texture());
    let hud = world.spawn();
    world.insert(hud, SceneId::new());
    world.insert(
        hud,
        UiRect::pinned(Vec2::new(1.0, 0.0), Vec2::ZERO, Vec2::new(64.0, 64.0)),
    );
    world.insert(
        hud,
        Sprite2D {
            texture: Some(texture()),
            ..Sprite2D::default()
        },
    );
    let Some((shot, _)) = shoot(app, "both-render-texture") else {
        return;
    };
    // The sprite: 108 px square centred 72 px left of the middle. The cube
    // fills about a third of the texture's height in its middle.
    let (sx, sy) = (WIDTH / 2 - 72, HEIGHT / 2);
    assert!(near(at(&shot, sx, sy), CUBE, 3), "{:?}", at(&shot, sx, sy));
    assert!(
        near(at(&shot, sx - 45, sy - 45), SKY, 3),
        "{:?}",
        at(&shot, sx - 45, sy - 45)
    );
    // The HUD image: the same picture, 64 px in the corner.
    assert!(
        near(at(&shot, WIDTH - 32, 32), CUBE, 3),
        "{:?}",
        at(&shot, WIDTH - 32, 32)
    );
    assert!(
        near(at(&shot, WIDTH - 60, 4), SKY, 3),
        "{:?}",
        at(&shot, WIDTH - 60, 4)
    );
    let in_hud = count(&shot, WIDTH - 64..WIDTH, 0..64, CUBE);
    assert!(
        (300..1000).contains(&in_hud),
        "{in_hud} cube pixels in the HUD"
    );
}

#[test]
fn a_split_screen_draws_3d_on_the_left_and_2d_on_the_right() {
    let mut app = App::default();
    let world = &mut app.world;
    camera_3d(
        world,
        Camera3D {
            viewport: Viewport::LEFT,
            ..Camera3D::default()
        },
    );
    cube(world, Vec3::ZERO);
    camera_2d(
        world,
        Vec2::ZERO,
        Camera2D {
            viewport: Viewport::RIGHT,
            ..Camera2D::new(5.0)
        },
    );
    sprite(world, Vec2::ZERO, 2.0, rgba(BLUE));
    let Some((shot, _)) = shoot(app, "both-split") else {
        return;
    };
    let half = WIDTH / 2;
    assert!(
        count(&shot, 0..half, 0..HEIGHT, CUBE) > 1000,
        "the cube on the left"
    );
    assert_eq!(count(&shot, half..WIDTH, 0..HEIGHT, CUBE), 0);
    let blue = count(&shot, half..WIDTH, 0..HEIGHT, BLUE);
    assert!(
        (blue as i32 - 36 * 36).abs() <= 80,
        "{blue}: the sprite on the right"
    );
    assert_eq!(count(&shot, 0..half, 0..HEIGHT, BLUE), 0);
    // The right half is not an overlay: the 2D camera's own background.
    assert!(!near(at(&shot, WIDTH - 5, 5), SKY, 3));
}
