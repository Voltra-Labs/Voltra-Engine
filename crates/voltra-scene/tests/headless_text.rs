//! A `Text` in pixels: glyph coverage reaches the frame as alpha, tinted by
//! the text's colour, through the same pipeline sprites draw with.
//!
//! The unit tests pin where glyph quads land and what the atlas image holds.
//! This pins the part only a GPU can show: that coverage sits in alpha and
//! not in RGB. With coverage in RGB and opaque alpha, the gap between an H's
//! stems would draw black over the clear colour instead of letting it
//! through, and the tint would multiply against grey.
//!
//! Skips itself when no GPU adapter is available.

use voltra_assets::{Fonts, Textures, DEFAULT_FONT};
use voltra_ecs::World;
use voltra_render::camera::{Camera2D, CameraBinding};
use voltra_render::glam::Vec2;
use voltra_render::pass::{self, MeshDraw};
use voltra_render::{pipeline, texture, wgpu, Texture};
use voltra_scene::sprite::sheets::Sheets;
use voltra_scene::text::{prepare, resolve_fonts};
use voltra_scene::{Align, SpriteBatch, Text, Transform};
use voltra_testkit::{copy_bundled_font, headless_device, read_texture, scratch_root, Rgba, CLEAR};

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Draws `batch` the way `App::redraw_*` does and reads the frame back.
fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    batch: &SpriteBatch,
    textures: &Textures,
    layout: &wgpu::BindGroupLayout,
) -> Vec<Rgba> {
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("headless-text-target"),
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

    // Four world units across the frame: sixteen pixels per unit.
    let camera = Camera2D::new(Vec2::ZERO, 0.5, 1.0);
    let camera_binding = CameraBinding::new(device);
    camera_binding.upload(queue, &camera);
    let white = Texture::white(device, queue).create_bind_group(device, layout);
    let render_pipeline =
        pipeline::create_flat_color(device, FORMAT, camera_binding.layout(), layout);

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
        label: Some("headless-text-encoder"),
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

fn is_red(pixel: Rgba) -> bool {
    pixel.r > 150 && pixel.g < 120 && pixel.b < 130
}

#[test]
fn coverage_lands_in_alpha_and_the_colour_tints_it() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let layout = texture::bind_group_layout(&device);
    let root = scratch_root();
    copy_bundled_font(&root, DEFAULT_FONT);
    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let mut fonts = Fonts::new(&root);

    // A big red H, centred across the frame, baseline a unit below centre.
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, Transform::from_translation(Vec2::new(0.0, -1.0)));
    world.insert(
        entity,
        Text {
            size: 3.0,
            color: [1.0, 0.0, 0.0, 1.0],
            align: Align::Center,
            ..Text::new("H")
        },
    );
    resolve_fonts(&mut world, &mut fonts);
    prepare(&world, &mut fonts);
    fonts.upload(&mut textures, &device, &queue);

    let sheets = Sheets {
        fonts: Some(&fonts),
        ..Sheets::default()
    };
    let batch = SpriteBatch::from_world(&world, sheets);
    assert_eq!(batch.sprite_count(), 1, "one glyph, one quad");

    let pixels = render(&device, &queue, &batch, &textures, &layout);

    // World y 0.5 — above the crossbar, below the top of the stems.
    let row: Vec<Rgba> = (0..SIZE)
        .map(|x| pixels[(24 * SIZE + x) as usize])
        .collect();
    let runs = row
        .windows(2)
        .filter(|pair| !is_red(pair[0]) && is_red(pair[1]))
        .count();
    assert_eq!(runs, 2, "two red stems across the row: {row:?}");

    let first = row.iter().position(|p| is_red(*p)).expect("a stem");
    let last = row.iter().rposition(|p| is_red(*p)).expect("a stem");
    let between = row[(first + last) / 2];
    assert!(
        between.is_clear_ish(),
        "the gap between the stems lets the clear colour through: {between:?}"
    );
    assert!(
        row[0].is_clear_ish(),
        "and so does the frame outside the glyph"
    );
}
