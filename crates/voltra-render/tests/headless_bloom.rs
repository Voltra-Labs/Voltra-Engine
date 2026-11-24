//! Bloom in pixels, through the render graph the renderer builds: a bright
//! square glows onto the dark around it, the far corners stay dark, and
//! with bloom off the chain is culled and the frame is the plain grade.
//!
//! Skips itself when no GPU adapter is available.

use voltra_render::camera::{Camera2D, CameraBinding};
use voltra_render::glam::Vec2;
use voltra_render::graph::{RenderGraph, Slot, TexturePool};
use voltra_render::pass::{self, MeshDraw};
use voltra_render::{
    pipeline, texture, wgpu, BloomPass, BloomSettings, Filter, Mesh, PostPass, PostSettings,
    RenderTarget, Texture, Vertex,
};
use voltra_testkit::{headless_device, read_texture, Rgba};

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The frame — a white square a quarter of the way across, on black —
/// graded with `post`, and how many textures the graph's pool ended with.
fn frame(post: PostSettings) -> Option<(Vec<Rgba>, usize)> {
    let (device, queue) = headless_device()?;
    let scene = RenderTarget::new(&device, "scene", FORMAT, SIZE, SIZE, Filter::Linear);
    let output = RenderTarget::new(&device, "output", FORMAT, SIZE, SIZE, Filter::Linear);
    let camera = CameraBinding::new(&device);
    camera.upload(&queue, &Camera2D::new(Vec2::ZERO, 1.0, 1.0));
    let layout = texture::bind_group_layout(&device);
    let white = Texture::white(&device, &queue).create_bind_group(&device, &layout);
    let sprites = pipeline::create_flat_color(&device, FORMAT, camera.layout(), &layout);
    let c = [1.0; 4];
    let h = 0.125;
    let square = [
        Vertex::new([-h, h], c, [0.0, 0.0]),
        Vertex::new([-h, -h], c, [0.0, 1.0]),
        Vertex::new([h, -h], c, [1.0, 1.0]),
        Vertex::new([h, h], c, [1.0, 0.0]),
    ];
    let mesh = Mesh::indexed(&device, "square", &square, &[0, 1, 2, 0, 2, 3]);

    let bloom = BloomPass::new(&device);
    let grade = PostPass::new(&device, FORMAT);
    let mut pool = TexturePool::default();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("bloom-test"),
    });
    pass::draw_mesh_batches(
        &mut encoder,
        scene.view(),
        &sprites,
        camera.bind_group(),
        Some(&mesh),
        &[MeshDraw {
            texture: &white,
            normal: None,
            light_layers: voltra_render::DEFAULT_LIGHT_LAYERS,
            indices: 0..6,
            material: None,
        }],
        wgpu::Color::BLACK,
    );

    let mut graph = RenderGraph::new();
    let source = graph.import(scene.view());
    let target = graph.import_output(output.view());
    let glow = bloom.add(&mut graph, &device, &queue, source, SIZE, SIZE, &post.bloom);
    let reads: Vec<Slot> = std::iter::once(source).chain(glow).collect();
    let (device_ref, queue_ref) = (&device, &queue);
    graph.add_pass("grade", &reads, &[target], move |encoder, views| {
        grade.record(
            device_ref,
            queue_ref,
            encoder,
            views.get(source),
            glow.map(|glow| views.get(glow)),
            views.get(target),
            &post,
            1.0,
        );
    });
    graph
        .run(&device, &mut encoder, &mut pool)
        .expect("a valid graph");
    queue.submit(Some(encoder.finish()));
    Some((
        read_texture(&device, &queue, output.texture(), SIZE, SIZE),
        pool.len(),
    ))
}

fn at(pixels: &[Rgba], x: u32, y: u32) -> Rgba {
    pixels[(y * SIZE + x) as usize]
}

#[test]
fn a_bright_square_glows_onto_the_dark_around_it() {
    let Some((plain, pool)) = frame(PostSettings::NEUTRAL) else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    assert_eq!(pool, 0, "bloom off: no level was ever made");
    let Some((bloomed, pool)) = frame(PostSettings {
        bloom: BloomSettings {
            intensity: 1.0,
            threshold: 0.5,
            ..BloomSettings::OFF
        },
        ..PostSettings::NEUTRAL
    }) else {
        return;
    };
    assert!(pool > 0, "the chain ran");

    // Just outside the square (which spans 24..40): black without bloom,
    // lit with it.
    let near_plain = at(&plain, 20, 32);
    let near_bloomed = at(&bloomed, 20, 32);
    assert!(near_plain.r < 5, "{near_plain:?}");
    assert!(
        near_bloomed.r > 30,
        "the glow reaches past the edge: {near_bloomed:?}"
    );
    // The far corner stays dark, and the square is still white.
    assert!(
        at(&bloomed, 1, 1).r < near_bloomed.r / 2,
        "{:?}",
        at(&bloomed, 1, 1)
    );
    assert!(at(&bloomed, 32, 32).r > 250);
}

#[test]
fn nothing_under_the_threshold_glows() {
    let Some((bloomed, _)) = frame(PostSettings {
        bloom: BloomSettings {
            intensity: 2.0,
            threshold: 4.0,
            softness: 0.0,
            ..BloomSettings::OFF
        },
        ..PostSettings::NEUTRAL
    }) else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    assert!(at(&bloomed, 20, 32).r < 5, "{:?}", at(&bloomed, 20, 32));
}
