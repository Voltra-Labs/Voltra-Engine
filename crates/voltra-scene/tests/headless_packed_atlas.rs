//! The texture packer's promise, in pixels: the sample sprites packed into
//! one sheet — trimmed, padded, extruded — draw exactly as the loose files
//! did, texel for texel, through the same sprite path a scene draws by.
//!
//! The loose files are drawn in the top half of the frame and their packed
//! regions in the bottom half, each laid out at the same place in its half,
//! one texel to one pixel; then every picture's rectangle is compared.
//!
//! Skips itself when no GPU adapter is available.

use std::path::Path;

use image::{Rgba, RgbaImage};
use voltra_assets::packer::{pack_folder, PackOptions};
use voltra_assets::{AssetPath, Atlases, Textures};
use voltra_render::camera::{CameraBinding, View2D};
use voltra_render::glam::{UVec2, Vec2};
use voltra_render::pass::{self, MeshDraw};
use voltra_render::{pipeline, texture, wgpu, Filter, Texture};
use voltra_scene::sprite::sheets::Sheets;
use voltra_scene::{Sprite, SpriteBatch, Transform};
use voltra_testkit::{headless_device, read_texture, scratch_root, Rgba as Pixel, CLEAR};

/// The frame, in pixels; the camera shows as many world units.
const SIZE: u32 = 256;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// A sprite with a transparent border on every side, uneven, so trimming
/// cuts something different off each edge.
fn badge() -> RgbaImage {
    RgbaImage::from_fn(22, 14, |x, y| {
        if (3..19).contains(&x) && (2..11).contains(&y) {
            Rgba([
                200,
                (x * 12) as u8,
                (y * 20) as u8,
                if x == 3 { 128 } else { 255 },
            ])
        } else {
            Rgba([0, 0, 0, 0])
        }
    })
}

/// Copies the engine's sample sprites, and the badge, into `folder`.
fn samples(folder: &Path) -> Vec<(String, UVec2)> {
    std::fs::create_dir_all(folder.join("more")).expect("the folder");
    let engine = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/sprites");
    let mut pictures = Vec::new();
    for name in ["checker", "coin", "ground"] {
        let from = engine.join(format!("{name}.png"));
        std::fs::copy(&from, folder.join(format!("{name}.png"))).expect("copied");
        let size = image::open(&from)
            .expect("a sample")
            .to_rgba8()
            .dimensions();
        pictures.push((name.to_owned(), UVec2::from(size)));
    }
    badge()
        .save(folder.join("more/badge.png"))
        .expect("the badge writes");
    pictures.push(("more/badge".to_owned(), UVec2::new(22, 14)));
    pictures
}

/// Where each picture's top-left corner goes in a half of the frame, in
/// world units from that half's top-left: left to right with a gap.
fn layout(pictures: &[(String, UVec2)]) -> Vec<Vec2> {
    let mut x = 0.0;
    pictures
        .iter()
        .map(|(_, size)| {
            let at = Vec2::new(x, 0.0);
            x += size.x as f32 + 4.0;
            at
        })
        .collect()
}

/// A transform putting a `size`-texel picture's top-left at `top_left`.
/// Its quad is centred on the transform, so the centre is half a picture in.
fn placed(top_left: Vec2, size: UVec2) -> Transform {
    let half = size.as_vec2() * 0.5;
    Transform::from_translation(Vec2::new(top_left.x + half.x, top_left.y - half.y))
}

fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    batch: &SpriteBatch,
    textures: &Textures,
) -> Vec<Pixel> {
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("packed-atlas-target"),
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
    // One world unit to one pixel: the view is `SIZE` units tall.
    let camera = View2D::new(Vec2::ZERO, 2.0 / SIZE as f32, 1.0);
    let camera_binding = CameraBinding::new(device);
    camera_binding.upload(queue, &camera);
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
            normal: None,
            light_layers: voltra_render::DEFAULT_LIGHT_LAYERS,
            primitives: range.primitives.clone(),
            material: None,
        })
        .collect();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("packed-atlas-encoder"),
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

/// The pixels of the `size` rectangle whose top-left is world `top_left`.
fn cut(pixels: &[Pixel], top_left: Vec2, size: UVec2) -> Vec<Pixel> {
    let x0 = (top_left.x + SIZE as f32 / 2.0) as u32;
    let y0 = (SIZE as f32 / 2.0 - top_left.y) as u32;
    (y0..y0 + size.y)
        .flat_map(|y| (x0..x0 + size.x).map(move |x| pixels[(y * SIZE + x) as usize]))
        .collect()
}

fn packed_draws_like_loose(filter: Filter) {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let root = scratch_root();
    let folder = root.join("sprites/samples");
    let pictures = samples(&folder);
    let atlas_file = root.join("sprites/samples.atlas.ron");
    let options = PackOptions {
        trim: true,
        ..PackOptions::default()
    };
    let report = pack_folder(&folder, &atlas_file, Some(&root), &options).expect("packs");
    assert_eq!(report.regions, pictures.len());

    let layout_bind = texture::bind_group_layout(&device);
    let mut textures = Textures::new(&device, &queue, &layout_bind, &root);
    textures.set_filter(filter);
    let mut atlases = Atlases::new(&root);
    let sheet = report.texture.clone().expect("the sheet is under the root");
    let atlas_path = AssetPath::new("sprites/samples.atlas.ron").expect("valid");

    let spots = layout(&pictures);
    let top = Vec2::new(-(SIZE as f32) / 2.0 + 2.0, SIZE as f32 / 2.0 - 2.0);
    let bottom = Vec2::new(top.x, -2.0);
    let mut batch = SpriteBatch::default();
    let mut sprites = Vec::new();
    for ((name, size), spot) in pictures.iter().zip(&spots) {
        let mut loose = Sprite {
            pixels_per_unit: Some(1.0),
            ..Sprite::default()
        };
        let path = AssetPath::new(&format!("sprites/samples/{name}.png")).expect("valid");
        loose.set_texture(Some(path), &mut textures, &device, &queue);
        sprites.push((placed(top + *spot, *size), loose));

        let mut region = Sprite {
            pixels_per_unit: Some(1.0),
            ..Sprite::default()
        };
        region.set_texture(Some(sheet.clone()), &mut textures, &device, &queue);
        region.set_atlas(Some(atlas_path.clone()), &mut atlases);
        let atlas = atlases.get(region.atlas_handle.expect("resolved"));
        region.frame = atlas.index_of(name).expect("a region per file");
        sprites.push((placed(bottom + *spot, *size), region));
    }
    let handle = atlases.load(&atlas_path);
    let trimmed = atlases.get(handle);
    let badge = trimmed.index_of("more/badge").expect("named");
    assert!(
        trimmed.meta(badge).and_then(|meta| meta.trim).is_some(),
        "the badge was trimmed, so this checks trimmed regions draw in place"
    );
    for (transform, sprite) in &sprites {
        batch.push(transform.matrix(), sprite, Sheets::new(&atlases, &textures));
    }

    let pixels = render(&device, &queue, &batch, &textures);

    for ((name, size), spot) in pictures.iter().zip(&spots) {
        let loose = cut(&pixels, top + *spot, *size);
        let packed = cut(&pixels, bottom + *spot, *size);
        assert!(
            loose.iter().any(|pixel| !pixel.is_clear_ish()),
            "{name}: the loose file drew nothing"
        );
        let differ = loose.iter().zip(&packed).filter(|(a, b)| a != b).count();
        assert_eq!(
            differ, 0,
            "{name} ({filter:?}): {differ} of {} pixels differ between the loose file and its region",
            loose.len()
        );
    }
}

#[test]
fn packed_regions_draw_pixel_identical_to_the_loose_files_nearest() {
    packed_draws_like_loose(Filter::Nearest);
}

#[test]
fn packed_regions_draw_pixel_identical_to_the_loose_files_linear() {
    packed_draws_like_loose(Filter::Linear);
}
