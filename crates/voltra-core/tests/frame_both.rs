//! One frame of both worlds, screenshotted headless: a 2D layer over a 3D
//! one, a 2D light map that lights only its own layer, a graded 2D
//! overlay's half-transparent pixels against the CPU, a 3D picture in a
//! picture over a 2D game, a 3D camera into a render texture shown on a
//! sprite and in the HUD, and a split screen of the two. Each skips
//! without a GPU adapter.
//!
//! Set `VOLTRA_SHOT_DIR` to a folder to keep the pictures.

use std::path::PathBuf;

use voltra_assets::{AssetPath, BuiltinMesh};
use voltra_core::{App, Headless, Screenshot, ScreenshotError};
use voltra_ecs::{Entity, World};
use voltra_render::glam::{Vec2, Vec3};
use voltra_render::render3d::Tonemap;
use voltra_scene::post::PostProcess;
use voltra_scene::{
    Camera2D, Camera3D, CameraClear, CameraTarget, MeshInstance3D, SceneId, Sprite2D, Transform2D,
    Transform3D, UiRect, Viewport,
};
use voltra_testkit::scratch_root;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 180;

/// The 3D clear every test draws against, linear.
const SKY: [f32; 3] = [0.05, 0.1, 0.3];
const CUBE: [f32; 3] = [0.9, 0.1, 0.05];
const BLUE: [f32; 3] = [0.1, 0.2, 1.0];
const GREEN: [f32; 3] = [0.1, 0.8, 0.1];

fn shoot(app: App, name: &str) -> Option<(Screenshot, Headless)> {
    let mut session = Headless::new(app);
    session.step(1, 1.0 / 60.0);
    let shot = match session.screenshot(WIDTH, HEIGHT) {
        Ok(shot) => shot,
        Err(ScreenshotError::NoDevice) => {
            eprintln!("no GPU adapter; skipping");
            return None;
        }
        Err(e) => panic!("{e}"),
    };
    if let Some(dir) = std::env::var_os("VOLTRA_SHOT_DIR") {
        let path = PathBuf::from(dir).join(format!("{name}.png"));
        shot.save_png(&path).expect("the picture is written");
    }
    Some((shot, session))
}

/// Linear to the 8-bit sRGB the screenshot holds.
fn encode(linear: [f32; 3]) -> [u8; 3] {
    linear.map(|c| {
        let c = c.clamp(0.0, 1.0);
        let s = if c <= 0.003_130_8 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0).round() as u8
    })
}

fn near(p: [u8; 4], linear: [f32; 3], by: i32) -> bool {
    let rgb = encode(linear);
    (0..3).all(|i| (i32::from(p[i]) - i32::from(rgb[i])).abs() <= by)
}

fn at(shot: &Screenshot, x: u32, y: u32) -> [u8; 4] {
    shot.pixel(x, y).expect("on the picture")
}

fn count(
    shot: &Screenshot,
    x: std::ops::Range<u32>,
    y: std::ops::Range<u32>,
    c: [f32; 3],
) -> usize {
    x.flat_map(|x| y.clone().map(move |y| (x, y)))
        .filter(|&(x, y)| near(at(shot, x, y), c, 3))
        .count()
}

/// A 3D camera 5 m back from the origin, flat colours, cleared to `SKY`.
fn camera_3d(world: &mut World, camera: Camera3D) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, Transform3D::from_translation(Vec3::new(0.0, 0.0, 5.0)));
    world.insert(
        e,
        Camera3D {
            tonemap: Tonemap::None,
            clear: CameraClear::Color([SKY[0], SKY[1], SKY[2], 1.0]),
            ..camera
        },
    );
    e
}

/// A 2 m cube at `at`, about 62 pixels tall from the camera above.
fn cube(world: &mut World, at: Vec3) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(
        e,
        Transform3D::from_translation(at).with_scale(Vec3::splat(2.0)),
    );
    world.insert(
        e,
        MeshInstance3D {
            color: [CUBE[0], CUBE[1], CUBE[2], 1.0],
            ..MeshInstance3D::new(BuiltinMesh::Cube)
        },
    );
    e
}

fn camera_2d(world: &mut World, at: Vec2, camera: Camera2D) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, Transform2D::from_translation(at));
    world.insert(e, camera);
    e
}

/// A 2D sprite `size` units across: 18 pixels a unit through a size-5
/// camera on 180 pixels.
fn sprite(world: &mut World, at: Vec2, size: f32, color: [f32; 4]) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(
        e,
        Transform2D::from_translation(at).with_scale(Vec2::splat(size)),
    );
    world.insert(e, Sprite2D::new(color));
    e
}

fn rgba(c: [f32; 3]) -> [f32; 4] {
    [c[0], c[1], c[2], 1.0]
}

#[test]
fn a_2d_sprite_draws_over_the_3d_cube_and_the_cube_shows_around_it() {
    let mut app = App::default();
    let world = &mut app.world;
    camera_3d(world, Camera3D::default());
    cube(world, Vec3::ZERO);
    // The 2D camera at the same priority: an overlay, transparent but for
    // its sprite, 2 units (36 pixels) square in the middle.
    camera_2d(world, Vec2::ZERO, Camera2D::new(5.0));
    sprite(world, Vec2::ZERO, 2.0, rgba(BLUE));
    let Some((shot, _)) = shoot(app, "both-sprite-over-cube") else {
        return;
    };
    let (cx, cy) = (WIDTH / 2, HEIGHT / 2);
    assert!(near(at(&shot, cx, cy), BLUE, 2), "{:?}", at(&shot, cx, cy));
    // Between the sprite's edge (18 px out) and the cube's (31 px out).
    for (x, y) in [(cx + 25, cy), (cx - 25, cy), (cx, cy + 25), (cx, cy - 25)] {
        assert!(
            near(at(&shot, x, y), CUBE, 2),
            "({x}, {y}): {:?}",
            at(&shot, x, y)
        );
    }
    // Past the cube: the 3D world's sky, the 2D layer transparent there.
    for (x, y) in [(5, 5), (WIDTH - 5, HEIGHT - 5), (cx + 60, cy)] {
        assert!(
            near(at(&shot, x, y), SKY, 2),
            "({x}, {y}): {:?}",
            at(&shot, x, y)
        );
    }
    let blue = count(&shot, 0..WIDTH, 0..HEIGHT, BLUE);
    assert!((blue as i32 - 36 * 36).abs() <= 80, "{blue} sprite pixels");
}

#[test]
fn a_2d_cameras_ambient_light_darkens_its_sprites_and_not_the_3d_world() {
    let mut app = App::default();
    let world = &mut app.world;
    camera_3d(world, Camera3D::default());
    cube(world, Vec3::ZERO);
    camera_2d(
        world,
        Vec2::ZERO,
        Camera2D {
            ambient: Some([0.25, 0.25, 0.25]),
            ..Camera2D::new(5.0)
        },
    );
    sprite(world, Vec2::new(-6.0, 0.0), 2.0, [1.0, 1.0, 1.0, 1.0]);
    let Some((shot, _)) = shoot(app, "both-ambient") else {
        return;
    };
    let (cx, cy) = (WIDTH / 2, HEIGHT / 2);
    // The 3D layer under the 2D light map: as bright as it was drawn.
    assert!(near(at(&shot, cx, cy), CUBE, 2), "{:?}", at(&shot, cx, cy));
    assert!(near(at(&shot, 5, 5), SKY, 2), "{:?}", at(&shot, 5, 5));
    // The 2D sprite, 6 units left (108 px): white times the ambient.
    let lit = at(&shot, cx - 108, cy);
    assert!(near(lit, [0.25; 3], 3), "{lit:?}");
}

/// Contrast about middle grey and saturation, as `grade.wgsl` does them.
fn grade(c: [f32; 3], contrast: f32, saturation: f32) -> [f32; 3] {
    let c = c.map(|v| (0.18 + (v - 0.18) * contrast).max(0.0));
    let luma = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    c.map(|v| (luma + (v - luma) * saturation).max(0.0))
}

#[test]
fn a_graded_2d_overlays_half_transparent_pixel_matches_the_cpu_with_no_halo() {
    let (contrast, saturation) = (0.5, 0.4);
    let sprite_colour = [1.0, 0.25, 0.05];
    let mut app = App::default();
    let world = &mut app.world;
    camera_3d(world, Camera3D::default());
    camera_2d(
        world,
        Vec2::ZERO,
        Camera2D {
            post: PostProcess {
                contrast,
                saturation,
                ..PostProcess::default()
            },
            ..Camera2D::new(5.0)
        },
    );
    // A soft edge, alpha 0.5 throughout, over the empty sky.
    sprite(
        world,
        Vec2::ZERO,
        4.0,
        [sprite_colour[0], sprite_colour[1], sprite_colour[2], 0.5],
    );
    let Some((shot, _)) = shoot(app, "both-graded-overlay") else {
        return;
    };
    // The reference: the colour graded straight, then laid over the sky by
    // its alpha — stored in 8 bits, 128/255.
    let alpha = 128.0 / 255.0;
    let graded = grade(sprite_colour, contrast, saturation);
    let reference: [f32; 3] = std::array::from_fn(|i| graded[i] * alpha + SKY[i] * (1.0 - alpha));
    let edge = at(&shot, WIDTH / 2, HEIGHT / 2);
    assert!(
        near(edge, reference, 1),
        "{edge:?}, the CPU says {:?}",
        encode(reference)
    );
    // Graded premultiplied instead, the edge would be off by far more
    // than the tolerance — the test can tell the two apart.
    let premultiplied = grade(sprite_colour.map(|c| c * alpha), contrast, saturation);
    let halo: [f32; 3] = std::array::from_fn(|i| premultiplied[i] + SKY[i] * (1.0 - alpha));
    assert!(
        !near(edge, halo, 6),
        "{edge:?} is the haloed {:?}",
        encode(halo)
    );
    // And where nothing is drawn the sky is untouched: a premultiplied
    // grade would lift a transparent pixel to grey (0.18 · (1 − 0.5)).
    for (x, y) in [(5, 5), (WIDTH - 5, 5), (WIDTH / 2, HEIGHT - 3)] {
        assert!(
            near(at(&shot, x, y), SKY, 0),
            "({x}, {y}): {:?}",
            at(&shot, x, y)
        );
    }
}

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
