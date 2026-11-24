//! Screen-space texts drawn headless and compared with reference images.
//!
//! The UI pass at one pixel per point, so glyph texels land 1:1 on the
//! frame and a reference is exact up to the rasterizer: lavapipe in CI and
//! on a developer's machine. A small share of differing pixels is allowed
//! for a GPU that rounds an edge the other way.
//!
//! `VOLTRA_BLESS=1` writes the references instead of comparing; a failed
//! comparison writes what was drawn beside the system's temp dir, to look
//! at.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use voltra_assets::{AssetPath, Atlases, Fonts, Textures};
use voltra_ecs::{Entity, World};
use voltra_render::camera::{screen_projection, CameraBinding};
use voltra_render::glam::Vec2;
use voltra_render::pass::{self, MeshDraw};
use voltra_render::{pipeline, texture, wgpu, Texture};
use voltra_scene::sprite::sheets::Sheets;
use voltra_scene::text::{prepare, resolve_fonts};
use voltra_scene::{ui, SceneId, SpriteBatch, Text, UiLayout, UiRect};
use voltra_testkit::{headless_device, read_texture, Rgba, CLEAR};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// A frame being set up: an asset root, its fonts, a world of texts.
pub struct Shot {
    pub root: PathBuf,
    pub fonts: Fonts,
    pub world: World,
    pub width: u32,
    pub height: u32,
}

impl Shot {
    /// A `width` × `height` frame over `root`, whose fonts fall back to
    /// `fallbacks` (asset paths).
    pub fn new(root: PathBuf, fallbacks: &[&str], width: u32, height: u32) -> Self {
        let mut fonts = Fonts::new(&root);
        let paths: Vec<AssetPath> = fallbacks
            .iter()
            .map(|p| AssetPath::new(p).expect("a valid path"))
            .collect();
        fonts.set_fallbacks(&paths);
        Self {
            root,
            fonts,
            world: World::new(),
            width,
            height,
        }
    }

    /// A text pinned at `at` (points from the top-left), `size` points wide.
    pub fn text(&mut self, at: Vec2, size: Vec2, text: Text) -> Entity {
        let entity = self.world.spawn();
        self.world.insert(entity, SceneId::new());
        self.world
            .insert(entity, UiRect::pinned(Vec2::ZERO, at, size));
        self.world.insert(entity, text);
        entity
    }

    /// The frame drawn, row by row from the top; `None` with no GPU.
    pub fn draw(&mut self) -> Option<Vec<Rgba>> {
        let (device, queue) = headless_device()?;
        let layout = texture::bind_group_layout(&device);
        let mut textures = Textures::new(&device, &queue, &layout, &self.root);
        let mut atlases = Atlases::new(&self.root);
        voltra_scene::text::icons::resolve_world(
            &mut self.world,
            &mut atlases,
            &mut textures,
            &device,
            &queue,
        );
        resolve_fonts(&mut self.world, &mut self.fonts);
        prepare(&self.world, &mut self.fonts, 1.0);
        self.fonts.upload(&mut textures, &device, &queue);

        let screen = Vec2::new(self.width as f32, self.height as f32);
        let ui_layout = UiLayout::from_world(&self.world, screen);
        let sheets = Sheets::new(&atlases, &textures).with_fonts(&self.fonts);
        let batch = ui::mesh::batch(&self.world, sheets, &ui_layout, 1.0);
        Some(render(
            &device,
            &queue,
            &batch,
            &textures,
            &layout,
            self.width,
            self.height,
        ))
    }
}

fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    batch: &SpriteBatch,
    textures: &Textures,
    layout: &wgpu::BindGroupLayout,
    width: u32,
    height: u32,
) -> Vec<Rgba> {
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("text-shot-target"),
        size: wgpu::Extent3d {
            width,
            height,
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
    let camera = CameraBinding::new(device);
    camera.upload_matrix(queue, screen_projection(width as f32, height as f32));
    let white = Texture::white(device, queue).create_bind_group(device, layout);
    let render_pipeline = pipeline::create_flat_color(device, FORMAT, camera.layout(), layout);
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
            indices: range.indices.clone(),
            material: None,
        })
        .collect();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("text-shot-encoder"),
    });
    pass::draw_mesh_batches(
        &mut encoder,
        &view,
        &render_pipeline,
        camera.bind_group(),
        mesh.as_ref(),
        &draws,
        CLEAR,
    );
    queue.submit(Some(encoder.finish()));
    read_texture(device, queue, &target, width, height)
}

/// The share of pixels that may differ from the reference.
const TOLERATED: f32 = 0.005;
/// How far apart two channels may be and still match.
const CHANNEL: u8 = 48;

/// Compares `pixels` with `tests/references/<name>.png`, or writes it there
/// under `VOLTRA_BLESS=1`.
pub fn matches_reference(name: &str, pixels: &[Rgba], width: u32, height: u32) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(format!("{name}.png"));
    let bytes: Vec<u8> = pixels.iter().flat_map(|p| [p.r, p.g, p.b, p.a]).collect();
    if std::env::var_os("VOLTRA_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
        save(&path, &bytes, width, height);
        return;
    }
    let reference = image::open(&path)
        .unwrap_or_else(|e| {
            panic!(
                "{}: {e}; run with VOLTRA_BLESS=1 to write it",
                path.display()
            )
        })
        .to_rgba8();
    assert_eq!(
        (reference.width(), reference.height()),
        (width, height),
        "{name}: the reference is another size"
    );
    let differing = reference
        .as_raw()
        .chunks(4)
        .zip(bytes.chunks(4))
        .filter(|(a, b)| {
            a.iter()
                .zip(b.iter())
                .any(|(x, y)| x.abs_diff(*y) > CHANNEL)
        })
        .count();
    let share = differing as f32 / (width * height) as f32;
    if share > TOLERATED {
        let actual = std::env::temp_dir().join(format!("{name}.actual.png"));
        save(&actual, &bytes, width, height);
        panic!(
            "{name}: {differing} pixels ({:.2}%) differ from {}; drawn: {}",
            share * 100.0,
            path.display(),
            actual.display()
        );
    }
}

fn save(path: &Path, rgba: &[u8], width: u32, height: u32) {
    use image::ImageEncoder;
    let file = std::fs::File::create(path).expect("the image file");
    image::codecs::png::PngEncoder::new(file)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .expect("the image encodes");
}

/// How many pixels of `pixels` are inked, not the clear colour.
pub fn inked(pixels: &[Rgba]) -> usize {
    pixels.iter().filter(|p| !p.is_clear_ish()).count()
}
