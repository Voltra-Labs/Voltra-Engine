//! A 3D scene screenshotted headless, as the player draws it: the shipped
//! `scenes/shapes3d.ron` — a camera, a floor, the built-in meshes
//! overlapping in depth, two walls a centimetre apart at 50 m — and a HUD
//! over it. Each skips without a GPU adapter.
//!
//! Set `VOLTRA_SHOT_DIR` to a folder to keep the pictures.

use std::path::PathBuf;

use voltra_assets::AssetPath;
use voltra_core::{App, Headless, Screenshot, ScreenshotError};
use voltra_ecs::World;
use voltra_render::glam::{Vec2, Vec3};
use voltra_scene::{Camera2D, SceneId, Sprite2D, Transform2D, UiRect};

const WIDTH: u32 = 480;
const HEIGHT: u32 = 270;

fn assets() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn open() -> Headless {
    let app = App::default().with_asset_root(assets());
    Headless::open(app, &AssetPath::new("scenes/shapes3d.ron").expect("valid"))
        .expect("the 3D sample loads")
}

fn shoot(session: &mut Headless, name: &str) -> Option<Screenshot> {
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
    Some(shot)
}

/// Linear to the 8-bit sRGB the screenshot holds.
fn encode(linear: [f32; 3]) -> [u8; 3] {
    linear.map(|c| {
        let s = if c <= 0.003_130_8 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0).round() as u8
    })
}

fn is(p: [u8; 4], rgb: [u8; 3]) -> bool {
    (0..3).all(|i| (i32::from(p[i]) - i32::from(rgb[i])).abs() <= 3)
}

fn count(shot: &Screenshot, linear: [f32; 3]) -> usize {
    let rgb = encode(linear);
    shot.rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| is(**p, rgb))
        .count()
}

/// The pixel `point` lands on through the scene's 3D camera.
fn pixel_of(world: &World, point: Vec3) -> (u32, u32) {
    let view = voltra_scene::camera3d::active_view(world, WIDTH as f32 / HEIGHT as f32)
        .expect("the scene has a 3D camera");
    let clip = view.view_projection() * point.extend(1.0);
    let ndc = clip / clip.w;
    let x = (ndc.x * 0.5 + 0.5) * WIDTH as f32;
    let y = (0.5 - ndc.y * 0.5) * HEIGHT as f32;
    (x as u32, y as u32)
}

const FLOOR: [f32; 3] = [0.35, 0.37, 0.4];
const CUBE: [f32; 3] = [0.9, 0.15, 0.1];
const SPHERE: [f32; 3] = [0.1, 0.8, 0.2];
const CAPSULE: [f32; 3] = [0.15, 0.3, 0.95];
const CYLINDER: [f32; 3] = [0.95, 0.8, 0.1];
const WALL_BEHIND: [f32; 3] = [0.0, 0.9, 0.9];
const WALL_FRONT: [f32; 3] = [0.9, 0.9, 0.9];

#[test]
fn the_shapes_draw_through_the_3d_camera_where_they_are() {
    let mut session = open();
    let Some(shot) = shoot(&mut session, "shapes3d") else {
        return;
    };
    for (name, colour, at_least) in [
        ("floor", FLOOR, 20_000),
        ("cube", CUBE, 800),
        ("sphere", SPHERE, 800),
        ("capsule", CAPSULE, 800),
        ("cylinder", CYLINDER, 800),
    ] {
        let n = count(&shot, colour);
        assert!(n >= at_least, "{name}: {n} pixels");
    }
    let world = session.world();
    for (name, point, colour) in [
        ("sphere", Vec3::new(-1.5, 0.6, 0.5), SPHERE),
        ("cylinder", Vec3::new(1.5, 0.4, 0.3), CYLINDER),
        ("capsule top", Vec3::new(0.8, 1.8, -0.5), CAPSULE),
        ("cube's far side", Vec3::new(-2.6, 0.7, -0.2), CUBE),
        ("floor", Vec3::new(0.0, 0.0, 2.0), FLOOR),
    ] {
        let (x, y) = pixel_of(world, point);
        let p = shot.pixel(x, y).expect("on screen");
        assert!(is(p, encode(colour)), "{name} at ({x}, {y}) is {p:?}");
    }
}

#[test]
fn two_walls_a_centimetre_apart_at_50_m_never_fight() {
    let mut session = open();
    let Some(shot) = shoot(&mut session, "shapes3d-walls") else {
        return;
    };
    let front = count(&shot, WALL_FRONT);
    assert!(front > 3_000, "the near wall: {front} pixels");
    assert_eq!(count(&shot, WALL_BEHIND), 0, "the far wall shows through");
}

#[test]
fn depth_not_file_order_decides_the_picture() {
    let mut session = open();
    let Some(first) = shoot(&mut session, "shapes3d-first") else {
        return;
    };
    // The same entities spawned again in reverse: the draw list is rebuilt
    // from a world whose order is the other way round.
    let world = session.world_mut();
    let mut all: Vec<_> = world
        .query::<voltra_scene::MeshInstance3D>()
        .map(|(e, _)| e)
        .collect();
    all.reverse();
    for entity in all {
        let mesh = world
            .get::<voltra_scene::MeshInstance3D>(entity)
            .cloned()
            .expect("queried");
        let at = *world
            .get::<voltra_scene::Transform3D>(entity)
            .expect("placed");
        world.despawn(entity);
        let again = world.spawn();
        world.insert(again, SceneId::new());
        world.insert(again, at);
        world.insert(again, mesh);
    }
    let Some(second) = shoot(&mut session, "shapes3d-reversed") else {
        return;
    };
    assert_eq!(first, second);
}

#[test]
fn the_hud_draws_over_the_3d_world() {
    let mut session = open();
    let world = session.world_mut();
    let panel = world.spawn();
    world.insert(panel, SceneId::new());
    world.insert(
        panel,
        UiRect::pinned(Vec2::ZERO, Vec2::ZERO, Vec2::new(60.0, 30.0)),
    );
    world.insert(panel, Sprite2D::new([1.0, 0.0, 1.0, 1.0]));
    let Some(shot) = shoot(&mut session, "shapes3d-hud") else {
        return;
    };
    let magenta = count(&shot, [1.0, 0.0, 1.0]);
    assert!(
        (magenta as i32 - 60 * 30).abs() <= 60,
        "{magenta} HUD pixels"
    );
}

#[test]
fn with_a_2d_camera_too_the_2d_camera_draws() {
    let mut session = open();
    let world = session.world_mut();
    let camera = world.spawn();
    world.insert(camera, SceneId::new());
    world.insert(camera, Transform2D::default());
    world.insert(camera, Camera2D::new(1.0));
    let block = world.spawn();
    world.insert(block, SceneId::new());
    world.insert(block, Transform2D::default().with_scale(Vec2::splat(4.0)));
    world.insert(block, Sprite2D::new([1.0, 0.0, 1.0, 1.0]));
    let Some(shot) = shoot(&mut session, "shapes3d-with-2d") else {
        return;
    };
    // The 2D block fills the view; no 3D mesh is drawn under it.
    assert_eq!(count(&shot, [1.0, 0.0, 1.0]), (WIDTH * HEIGHT) as usize);
}
