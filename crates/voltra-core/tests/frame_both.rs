//! One frame of both worlds, screenshotted headless, as layers: a 2D layer
//! over a 3D one, a 2D light map that lights only its own layer, and a
//! graded 2D overlay's half-transparent pixels against the CPU. Each skips
//! without a GPU adapter.
//!
//! Set `VOLTRA_SHOT_DIR` to a folder to keep the pictures.

mod both;

use both::*;

use voltra_core::App;
use voltra_render::glam::{Vec2, Vec3};
use voltra_scene::post::PostProcess;
use voltra_scene::{Camera2D, Camera3D};

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
