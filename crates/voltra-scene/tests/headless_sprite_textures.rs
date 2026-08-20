//! The whole chain, in pixels: a `Sprite`'s path becomes a handle, the handle
//! becomes a bind group, the batch's ranges become draw calls, and the right
//! texels land in the right half of the frame.
//!
//! Every link in that chain is already unit-tested on its own. This exists
//! because the links are what break: a bind group built against the wrong
//! layout, a range off by one quad, or a `None` run drawn against the
//! placeholder instead of white are all invisible to those tests and obvious
//! here.
//!
//! Skips itself when no GPU adapter is available.

use std::path::Path;

use voltra_assets::{AssetPath, Textures};
use voltra_render::camera::{Camera2D, CameraBinding};
use voltra_render::glam::Vec2;
use voltra_render::pass::{self, MeshDraw};
use voltra_render::{pipeline, texture, wgpu, Texture};
use voltra_scene::sprite::sheets::Sheets;
use voltra_scene::{Sprite, SpriteBatch, Transform};
use voltra_testkit::{headless_device, read_texture, scratch_root, Rgba, CLEAR};

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

macro_rules! device_or_skip {
    () => {
        match headless_device() {
            Some(pair) => pair,
            None => {
                eprintln!("no GPU adapter; skipping");
                return;
            }
        }
    };
}

/// Writes an opaque single-colour PNG at `root/name`.
///
/// A flat colour rather than `voltra_testkit::write_png`'s red: these tests
/// tell two textures apart by the colour that comes back, so each needs its
/// own.
fn write_flat_png(root: &Path, name: &str, rgba: [u8; 4]) {
    use image::ImageEncoder;

    let path = root.join(name);
    std::fs::create_dir_all(path.parent().expect("the PNG has a parent")).expect("asset subdir");

    let pixels: Vec<u8> = (0..16 * 16).flat_map(|_| rgba).collect();
    let file = std::fs::File::create(&path).expect("creating the PNG");
    image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
        .write_image(&pixels, 16, 16, image::ExtendedColorType::Rgba8)
        .expect("encoding the PNG");
}

/// Draws a batch exactly the way `App::redraw_*` does, and reads the frame back.
///
/// The mapping from `SpriteRange` to `MeshDraw` is deliberately the same shape
/// as `voltra_core::app::mesh_draws`: this test is worthless if it draws the
/// batch differently from the engine.
fn render_batch(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    batch: &SpriteBatch,
    textures: &Textures,
    camera: &Camera2D,
) -> Vec<Rgba> {
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("headless-sprite-target"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let camera_binding = CameraBinding::new(device);
    camera_binding.upload(queue, camera);
    let layout = texture::bind_group_layout(device);
    let white = Texture::white(device, queue).create_bind_group(device, &layout);
    let render_pipeline =
        pipeline::create_flat_color(device, FORMAT, camera_binding.layout(), &layout);

    let mesh = batch.upload(device);
    let draws: Vec<MeshDraw> = batch
        .ranges
        .iter()
        .map(|range| MeshDraw {
            texture: match range.texture {
                Some(handle) => textures.bind_group(handle),
                None => &white,
            },
            indices: range.indices.clone(),
        })
        .collect();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("headless-sprite-encoder"),
    });
    pass::draw_mesh_batches(
        &mut encoder,
        &view,
        &render_pipeline,
        camera_binding.bind_group(),
        mesh.as_ref(),
        &draws,
        CLEAR,
    );
    queue.submit(Some(encoder.finish()));

    read_texture(device, queue, &target, SIZE, SIZE)
}

fn at(pixels: &[Rgba], x: u32, y: u32) -> Rgba {
    pixels[(y * SIZE + x) as usize]
}

/// A camera wide enough that two sprites a unit apart land in opposite halves.
///
/// Built through `Camera2D::new`: `zoom` is a private field with a clamping
/// setter, precisely so nobody writes a zero into it.
///
/// Zoom 0.25 (the plan's original value) makes the visible region 8 world
/// units wide, so a sprite one unit wide sitting at x = -1 covers ndc x in
/// [-0.375, -0.125] — short of the ndc -0.5 sampled at pixel `SIZE / 4` and
/// nothing lands there. 0.4 shrinks the visible region enough that both
/// sprites' quads straddle their sampled column while still leaving a clear
/// gap between them at the centre.
fn wide_camera() -> Camera2D {
    Camera2D::new(Vec2::ZERO, 0.4, 1.0)
}

/// Tight enough that one sprite at the origin fills most of the frame.
fn close_camera() -> Camera2D {
    Camera2D::new(Vec2::ZERO, 0.5, 1.0)
}

/// A sprite at `x`, optionally naming `path`, resolved through `textures`.
fn sprite_at(
    x: f32,
    path: Option<&str>,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (Transform, Sprite) {
    let mut sprite = Sprite::default();
    if let Some(path) = path {
        sprite.set_texture(
            Some(AssetPath::new(path).expect("a valid asset path")),
            textures,
            device,
            queue,
        );
    }
    (Transform::from_translation(Vec2::new(x, 0.0)), sprite)
}

#[test]
fn two_sprites_naming_one_path_sample_the_same_texture() {
    let (device, queue) = device_or_skip!();
    let layout = texture::bind_group_layout(&device);
    let root = scratch_root();
    write_flat_png(&root, "sprites/green.png", [40, 200, 90, 255]);

    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let left = sprite_at(
        -1.0,
        Some("sprites/green.png"),
        &mut textures,
        &device,
        &queue,
    );
    let right = sprite_at(
        1.0,
        Some("sprites/green.png"),
        &mut textures,
        &device,
        &queue,
    );

    assert_eq!(
        left.1.texture_handle, right.1.texture_handle,
        "one path must resolve to one handle"
    );

    let mut batch = SpriteBatch::default();
    batch.push(left.0.matrix(), &left.1, Sheets::default());
    batch.push(right.0.matrix(), &right.1, Sheets::default());
    assert_eq!(batch.ranges.len(), 1, "one texture must be one run");

    let pixels = render_batch(&device, &queue, &batch, &textures, &wide_camera());
    let left_px = at(&pixels, SIZE / 4, SIZE / 2);
    let right_px = at(&pixels, SIZE * 3 / 4, SIZE / 2);

    assert!(!left_px.is_clear_ish(), "nothing drawn on the left");
    assert!(!right_px.is_clear_ish(), "nothing drawn on the right");
    assert!(
        left_px.g > left_px.r && left_px.g > left_px.b,
        "left should sample the green PNG, got {left_px:?}"
    );
    assert_eq!(
        left_px, right_px,
        "both sprites sample one texture, so both halves must match"
    );
}

#[test]
fn two_paths_reach_their_own_textures_in_one_frame() {
    let (device, queue) = device_or_skip!();
    let layout = texture::bind_group_layout(&device);
    let root = scratch_root();
    write_flat_png(&root, "green.png", [40, 200, 90, 255]);
    write_flat_png(&root, "blue.png", [50, 90, 220, 255]);

    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let left = sprite_at(-1.0, Some("green.png"), &mut textures, &device, &queue);
    let right = sprite_at(1.0, Some("blue.png"), &mut textures, &device, &queue);

    let mut batch = SpriteBatch::default();
    batch.push(left.0.matrix(), &left.1, Sheets::default());
    batch.push(right.0.matrix(), &right.1, Sheets::default());
    assert_eq!(batch.ranges.len(), 2, "two textures must be two runs");

    let pixels = render_batch(&device, &queue, &batch, &textures, &wide_camera());
    let left_px = at(&pixels, SIZE / 4, SIZE / 2);
    let right_px = at(&pixels, SIZE * 3 / 4, SIZE / 2);

    assert!(
        left_px.g > left_px.b,
        "left half must be the green PNG, got {left_px:?}"
    );
    assert!(
        right_px.b > right_px.g,
        "right half must be the blue PNG, got {right_px:?}"
    );
}

#[test]
fn a_path_that_does_not_load_draws_the_placeholder_checker() {
    let (device, queue) = device_or_skip!();
    let layout = texture::bind_group_layout(&device);
    let root = scratch_root();

    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let missing = sprite_at(0.0, Some("absent.png"), &mut textures, &device, &queue);
    assert!(
        missing.1.texture_handle.is_some(),
        "a failed load must still resolve to something drawable"
    );
    assert_ne!(
        missing.1.texture_handle,
        Some(textures.placeholder()),
        "a failed path gets its own slot so hot reload can fix it in place"
    );

    let mut batch = SpriteBatch::default();
    batch.push(missing.0.matrix(), &missing.1, Sheets::default());

    let pixels = render_batch(&device, &queue, &batch, &textures, &close_camera());

    // The placeholder is an 8x8 magenta-and-black checker, so somewhere in
    // the frame holds a strongly magenta pixel and somewhere else a
    // near-black one. A single sample cannot tell a checker from a flat
    // fill; two must disagree.
    //
    // Checked against the whole frame rather than an `!is_clear_ish()`
    // pre-filter: the checker's black cells are themselves within
    // `is_clear_ish`'s thresholds (it exists to match the dark clear colour,
    // and 0,0,0 satisfies the same bound), so filtering by it first would
    // throw the black half of the checker away before the dark-texel check
    // ever saw it. The clear colour itself (r=g=89, b=97) is comfortably
    // above the strict thresholds below, so it cannot be mistaken for either.
    assert!(
        pixels.iter().any(|px| !px.is_clear_ish()),
        "the missing sprite drew nothing"
    );
    assert!(
        pixels
            .iter()
            .any(|px| px.r > 180 && px.b > 180 && px.g < 100),
        "no magenta texel in the frame"
    );
    assert!(
        pixels.iter().any(|px| px.r < 80 && px.g < 80 && px.b < 80),
        "no dark texel in the frame, so it is not a checker"
    );
}

#[test]
fn an_untextured_sprite_still_tints_through_white() {
    let (device, queue) = device_or_skip!();
    let layout = texture::bind_group_layout(&device);
    let root = scratch_root();

    let textures = Textures::new(&device, &queue, &layout, &root);
    let sprite = Sprite::new([0.9, 0.2, 0.2, 1.0]);
    assert!(sprite.texture_handle.is_none());

    let mut batch = SpriteBatch::default();
    batch.push(Transform::default().matrix(), &sprite, Sheets::default());
    assert_eq!(batch.ranges.len(), 1);
    assert!(
        batch.ranges[0].texture.is_none(),
        "an untextured sprite must produce a None run, not the placeholder"
    );

    let pixels = render_batch(&device, &queue, &batch, &textures, &close_camera());
    let centre = at(&pixels, SIZE / 2, SIZE / 2);

    assert!(
        centre.r > centre.g && centre.r > centre.b,
        "the sprite colour must survive the white texture, got {centre:?}"
    );
    assert!(!centre.is_clear_ish(), "nothing was drawn");
}

#[test]
fn a_reloaded_texture_changes_what_is_drawn() {
    // The dimension assertions in voltra-assets prove the slot was replaced.
    // This proves the replacement reaches the screen — which it only does if
    // the bind group was replaced too, because the old one names the old view.
    let (device, queue) = device_or_skip!();
    let layout = texture::bind_group_layout(&device);
    let root = scratch_root();
    write_flat_png(&root, "swap.png", [40, 200, 90, 255]);

    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let sprite = sprite_at(0.0, Some("swap.png"), &mut textures, &device, &queue);

    let mut batch = SpriteBatch::default();
    batch.push(sprite.0.matrix(), &sprite.1, Sheets::default());

    let before = render_batch(&device, &queue, &batch, &textures, &close_camera());
    let centre = at(&before, SIZE / 2, SIZE / 2);
    assert!(centre.g > centre.b, "the green PNG first: {centre:?}");

    write_flat_png(&root, "swap.png", [50, 90, 220, 255]);
    let path = AssetPath::new("swap.png").expect("a valid asset path");
    assert!(textures.reload(&device, &queue, &path));

    // The same batch, the same sprite, the same handle. Nothing in the world
    // was touched — only the texture behind the handle.
    let after = render_batch(&device, &queue, &batch, &textures, &close_camera());
    let centre = at(&after, SIZE / 2, SIZE / 2);
    assert!(
        centre.b > centre.g,
        "the reload must reach the screen: {centre:?}"
    );
}
