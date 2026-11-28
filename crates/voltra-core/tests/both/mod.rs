//! What the one-frame-of-both-worlds tests share: a 3D camera on a cube,
//! 2D cameras and sprites, a screenshot and colours read off it.

#![allow(dead_code)]

use std::path::PathBuf;

use voltra_assets::BuiltinMesh;
use voltra_core::{App, Headless, Screenshot, ScreenshotError};
use voltra_ecs::{Entity, World};
use voltra_render::glam::{Vec2, Vec3};
use voltra_render::render3d::Tonemap;
use voltra_scene::{
    Camera2D, Camera3D, CameraClear, MeshInstance3D, SceneId, Sprite2D, Transform2D, Transform3D,
};

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 180;

/// The 3D clear every test draws against, linear.
pub const SKY: [f32; 3] = [0.05, 0.1, 0.3];
pub const CUBE: [f32; 3] = [0.9, 0.1, 0.05];
pub const BLUE: [f32; 3] = [0.1, 0.2, 1.0];
pub const GREEN: [f32; 3] = [0.1, 0.8, 0.1];

pub fn shoot(app: App, name: &str) -> Option<(Screenshot, Headless)> {
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
pub fn encode(linear: [f32; 3]) -> [u8; 3] {
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

pub fn near(p: [u8; 4], linear: [f32; 3], by: i32) -> bool {
    let rgb = encode(linear);
    (0..3).all(|i| (i32::from(p[i]) - i32::from(rgb[i])).abs() <= by)
}

pub fn at(shot: &Screenshot, x: u32, y: u32) -> [u8; 4] {
    shot.pixel(x, y).expect("on the picture")
}

pub fn count(
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
pub fn camera_3d(world: &mut World, camera: Camera3D) -> Entity {
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

/// The shipped white unlit material: a mesh in its own flat colour.
pub fn unlit() -> voltra_assets::AssetPath {
    voltra_assets::AssetPath::new("materials/unlit.material.ron").expect("a path")
}

/// A 2 m cube at `at`, about 62 pixels tall from the camera above, unlit.
pub fn cube(world: &mut World, at: Vec3) -> Entity {
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
            ..MeshInstance3D::new(BuiltinMesh::Cube).with_material(unlit())
        },
    );
    e
}

pub fn camera_2d(world: &mut World, at: Vec2, camera: Camera2D) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, Transform2D::from_translation(at));
    world.insert(e, camera);
    e
}

/// A 2D sprite `size` units across: 18 pixels a unit through a size-5
/// camera on 180 pixels.
pub fn sprite(world: &mut World, at: Vec2, size: f32, color: [f32; 4]) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(
        e,
        Transform2D::from_translation(at).with_scale(Vec2::splat(size)),
    );
    world.insert(e, Sprite2D::new(color));
    e
}

pub fn rgba(c: [f32; 3]) -> [f32; 4] {
    [c[0], c[1], c[2], 1.0]
}
