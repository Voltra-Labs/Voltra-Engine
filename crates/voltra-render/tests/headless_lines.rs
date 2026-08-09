//! Lines, in pixels.
//!
//! The three questions the unit tests in `lines.rs` cannot answer: does the
//! width reach the screen, is it a count of pixels rather than of world units,
//! and does the pass leave what was under it alone.
//!
//! Skips itself when no GPU adapter is available.

use voltra_render::glam::Vec2;
use voltra_render::wgpu;
use voltra_render::{lines::LineBatch, pass, pipeline, Camera2D, CameraBinding};
use voltra_testkit::{headless_device, read_texture, Rgba, CLEAR};

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

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

/// Renders one batch over a cleared target and reads the frame back.
fn render(device: &wgpu::Device, queue: &wgpu::Queue, batch: &LineBatch, zoom: f32) -> Vec<Rgba> {
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("headless-lines-target"),
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
    camera_binding.upload(queue, &Camera2D::new(Vec2::ZERO, zoom, 1.0));

    let viewport_layout = pipeline::viewport_bind_group_layout(device);
    let viewport = pipeline::viewport_binding(device, queue, &viewport_layout, SIZE, SIZE);
    let line_pipeline =
        pipeline::create_lines(device, FORMAT, camera_binding.layout(), &viewport_layout);

    let mesh = batch.upload(device);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("headless-lines-encoder"),
    });
    // A clear first, in its own pass, because `draw_lines` deliberately does
    // not clear — that is what makes it an overlay. With no mesh and no draws
    // this records the clear and nothing else, so the pipeline it is handed is
    // never bound.
    pass::draw_mesh_batches(
        &mut encoder,
        &view,
        &line_pipeline,
        camera_binding.bind_group(),
        None,
        &[],
        CLEAR,
    );
    pass::draw_lines(
        &mut encoder,
        &view,
        &line_pipeline,
        camera_binding.bind_group(),
        &viewport,
        mesh.as_ref(),
    );
    queue.submit(Some(encoder.finish()));

    read_texture(device, queue, &target, SIZE, SIZE)
}

/// Pixels that are not the clear colour.
fn painted(pixels: &[Rgba]) -> usize {
    pixels.iter().filter(|px| !px.is_clear_ish()).count()
}

#[test]
fn a_wider_line_paints_more_pixels() {
    let (device, queue) = device_or_skip!();

    let mut thin = LineBatch::default();
    thin.push(Vec2::new(-0.5, 0.0), Vec2::new(0.5, 0.0), 1.0, WHITE);
    let mut thick = LineBatch::default();
    thick.push(Vec2::new(-0.5, 0.0), Vec2::new(0.5, 0.0), 5.0, WHITE);

    let thin_px = painted(&render(&device, &queue, &thin, 0.5));
    let thick_px = painted(&render(&device, &queue, &thick, 0.5));

    assert!(thin_px > 0, "the thin line drew nothing");
    assert!(
        thick_px > thin_px * 2,
        "5px must paint far more than 1px: {thick_px} vs {thin_px}"
    );
}

#[test]
fn the_width_is_pixels_not_world_units() {
    // The same segment at two zooms. Its *length* on screen changes; its
    // *thickness* must not, which is the whole reason the widening happens
    // after projection rather than before.
    let (device, queue) = device_or_skip!();

    let mut batch = LineBatch::default();
    batch.push(Vec2::new(0.0, -40.0), Vec2::new(0.0, 40.0), 4.0, WHITE);

    // Vertical and far longer than the frame at both zooms, so it spans every
    // row in each and only the column count can differ.
    let near = render(&device, &queue, &batch, 0.5);
    let far = render(&device, &queue, &batch, 0.25);

    let row = (SIZE / 2) as usize;
    let width_of = |px: &[Rgba]| {
        (0..SIZE as usize)
            .filter(|x| !px[row * SIZE as usize + x].is_clear_ish())
            .count()
    };

    let near_w = width_of(&near);
    let far_w = width_of(&far);
    assert!(near_w > 0, "nothing drawn");
    assert_eq!(
        near_w, far_w,
        "thickness changed with zoom: {near_w} vs {far_w}"
    );
}

#[test]
fn the_line_pass_does_not_erase_what_was_under_it() {
    let (device, queue) = device_or_skip!();

    let mut batch = LineBatch::default();
    batch.push(Vec2::new(-0.5, 0.0), Vec2::new(0.5, 0.0), 3.0, WHITE);

    let pixels = render(&device, &queue, &batch, 0.5);

    // The clear colour has to still be in the corners. Had `draw_lines`
    // cleared, the corners would hold whatever it cleared to and the line
    // would be the only thing left.
    assert!(pixels[0].is_clear_ish(), "corner was overwritten");
    assert!(painted(&pixels) > 0, "the line itself is missing");
}

#[test]
fn an_empty_batch_draws_nothing_and_does_not_panic() {
    let (device, queue) = device_or_skip!();

    let pixels = render(&device, &queue, &LineBatch::default(), 0.5);

    assert_eq!(painted(&pixels), 0);
}
