//! Several cameras in one frame, screenshotted headless: a two-player
//! split screen with a minimap drawn into a render texture and shown in
//! the corner, and a pixel-perfect camera checked against a reference
//! worked out pixel by pixel. Each skips without a GPU adapter.

use std::path::Path;

use voltra_core::{App, Headless, Screenshot, ScreenshotError};
use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_render::Filter;
use voltra_scene::{Camera, CameraTarget, SceneId, Sprite, Transform, UiRect, Viewport};
use voltra_testkit::scratch_root;

fn shoot(session: &mut Headless, width: u32, height: u32) -> Option<Screenshot> {
    match session.screenshot(width, height) {
        Ok(shot) => Some(shot),
        Err(ScreenshotError::NoDevice) => {
            eprintln!("no GPU adapter; skipping");
            None
        }
        Err(e) => panic!("{e}"),
    }
}

fn block(world: &mut World, at: Vec2, color: [f32; 4]) {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(
        e,
        Transform::from_translation(at).with_scale(Vec2::splat(3.0)),
    );
    world.insert(e, Sprite::new(color));
}

fn camera(world: &mut World, at: Vec2, camera: Camera) -> voltra_ecs::Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, Transform::from_translation(at));
    world.insert(e, camera);
    e
}

fn is(p: [u8; 4], rgb: [u8; 3]) -> bool {
    (0..3).all(|i| (i32::from(p[i]) - i32::from(rgb[i])).abs() <= 4)
}

fn count(
    shot: &Screenshot,
    x: std::ops::Range<u32>,
    y: std::ops::Range<u32>,
    rgb: [u8; 3],
) -> usize {
    x.flat_map(|x| y.clone().map(move |y| (x, y)))
        .filter(|&(x, y)| is(shot.pixel(x, y).expect("inside"), rgb))
        .count()
}

#[test]
fn a_split_screen_and_a_minimap_draw_in_one_frame() {
    let root = scratch_root();
    std::fs::write(
        root.join("minimap.rendertexture.ron"),
        "(version: 1, width: 64, height: 32, nearest: true)",
    )
    .expect("the render texture");
    let mut app = App::default().with_asset_root(&root);
    let world = &mut app.world;
    // Player one's red block far left, player two's blue far right.
    block(world, Vec2::new(-20.0, 0.0), [1.0, 0.0, 0.0, 1.0]);
    block(world, Vec2::new(20.0, 0.0), [0.0, 0.0, 1.0, 1.0]);
    camera(
        world,
        Vec2::new(-20.0, 0.0),
        Camera {
            size: 2.0,
            viewport: Viewport::LEFT,
            ..Camera::default()
        },
    );
    camera(
        world,
        Vec2::new(20.0, 0.0),
        Camera {
            size: 2.0,
            viewport: Viewport::RIGHT,
            ..Camera::default()
        },
    );
    // The minimap sees both, into a texture a HUD image shows top centre.
    let map = camera(world, Vec2::ZERO, Camera::new(12.0));
    world.insert(
        map,
        CameraTarget::new(
            voltra_assets::AssetPath::new("minimap.rendertexture.ron").expect("path"),
        ),
    );
    let image = world.spawn();
    world.insert(image, SceneId::new());
    world.insert(
        image,
        UiRect::pinned(Vec2::new(0.5, 0.0), Vec2::ZERO, Vec2::new(64.0, 32.0)),
    );
    world.insert(
        image,
        Sprite {
            texture: Some(
                voltra_assets::AssetPath::new("minimap.rendertexture.ron").expect("path"),
            ),
            ..Sprite::default()
        },
    );

    let mut session = Headless::new(app);
    session.step(1, 1.0 / 60.0);
    let Some(shot) = shoot(&mut session, 320, 160) else {
        return;
    };
    let (red, blue) = ([255, 0, 0], [0, 0, 255]);
    // Below the minimap: the halves, each its own player's block.
    let left_red = count(&shot, 40..120, 60..150, red);
    let right_blue = count(&shot, 200..280, 60..150, blue);
    assert!(left_red > 5000, "the left half sees red: {left_red}");
    assert!(right_blue > 5000, "the right half sees blue: {right_blue}");
    assert_eq!(
        count(&shot, 0..160, 40..160, blue),
        0,
        "no blue on the left"
    );
    assert_eq!(
        count(&shot, 160..320, 40..160, red),
        0,
        "no red on the right"
    );
    // The minimap, 64 × 32 at the top centre: both blocks, small, red left
    // of blue.
    let map_red = count(&shot, 128..160, 0..32, red);
    let map_blue = count(&shot, 160..192, 0..32, blue);
    assert!(
        map_red >= 8 && map_blue >= 8,
        "the minimap shows both: {map_red} {map_blue}"
    );
}

/// A 4 × 4 image, every texel its own colour, as an sRGB PNG.
fn write_tiles(root: &Path) -> Vec<[u8; 3]> {
    use image::ImageEncoder;
    let colours: Vec<[u8; 3]> = (0..16u8)
        .map(|i| [40 + i * 13, 200 - i * 11, if i % 2 == 0 { 30 } else { 220 }])
        .collect();
    let pixels: Vec<u8> = colours
        .iter()
        .flat_map(|c| [c[0], c[1], c[2], 255])
        .collect();
    let file = std::fs::File::create(root.join("tiles.png")).expect("the PNG");
    image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
        .write_image(&pixels, 4, 4, image::ExtendedColorType::Rgba8)
        .expect("encoded");
    colours
}

#[test]
fn a_pixel_perfect_camera_matches_its_reference_pixel_for_pixel() {
    let root = scratch_root();
    let colours = write_tiles(&root);
    let mut app = App::default()
        .with_asset_root(&root)
        .with_texture_filter(Filter::Nearest);
    let tiles = app.world.spawn();
    app.world.insert(tiles, SceneId::new());
    app.world.insert(tiles, Transform::default());
    app.world.insert(
        tiles,
        Sprite {
            texture: Some(voltra_assets::AssetPath::new("tiles.png").expect("path")),
            ..Sprite::default()
        },
    );
    // Two units over 64 pixels: 32 pixels a unit, 8 a texel. The position
    // is off the pixel grid; pixel-perfect snaps it.
    let at = Vec2::new(0.013, -0.021);
    camera(
        &mut app.world,
        at,
        Camera {
            size: 1.0,
            pixel_perfect: true,
            ..Camera::default()
        },
    );
    let mut session = Headless::new(app);
    session.step(1, 1.0 / 60.0);
    let Some(shot) = shoot(&mut session, 64, 64) else {
        return;
    };

    // The reference: the view's edges snapped to whole pixels, each pixel's
    // centre mapped to the texel it lands in.
    let pixel = 1.0 / 32.0;
    let snap = |c: f32| ((c - 1.0) / pixel).round() * pixel + 1.0;
    let centre = Vec2::new(snap(at.x), snap(at.y));
    let mut checked = 0;
    for row in 0..64u32 {
        for column in 0..64u32 {
            let world = Vec2::new(
                centre.x - 1.0 + (column as f32 + 0.5) * pixel,
                centre.y + 1.0 - (row as f32 + 0.5) * pixel,
            );
            if world.x.abs() >= 0.5 || world.y.abs() >= 0.5 {
                continue;
            }
            let tx = ((world.x + 0.5) * 4.0).floor() as usize;
            let ty = ((0.5 - world.y) * 4.0).floor() as usize;
            let expected = colours[ty * 4 + tx];
            let got = shot.pixel(column, row).expect("inside");
            assert!(
                (0..3).all(|i| (i32::from(got[i]) - i32::from(expected[i])).abs() <= 2),
                "pixel ({column}, {row}) is {got:?}, texel ({tx}, {ty}) is {expected:?}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 32 * 32, "every pixel of the sprite checked");
}
