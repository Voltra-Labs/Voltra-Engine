//! Light layers in pixels: two white quads, the left on light layer 1 and
//! the right on layer 2, a lamp in the middle and a dim ambient. A lamp of
//! layer 2 lights only the right; a lamp of every layer lights both; the
//! background, drawn by nothing, is on every layer.
//!
//! Skips itself when no GPU adapter is available.

use voltra_render::camera::{Camera2D, CameraBinding};
use voltra_render::glam::Vec2;
use voltra_render::graph::{RenderGraph, TexturePool};
use voltra_render::light_layers::LAYER_FORMAT;
use voltra_render::{
    pass, pipeline, texture, wgpu, Filter, LayerPass, LightPass, Lighting, Mesh, MeshDraw,
    PointLight, PostPass, PostSettings, RenderTarget, SceneDraw, Texture, Vertex, ALL_LIGHT_LAYERS,
};
use voltra_testkit::{headless_device, read_texture, Rgba};

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Two quads filling the left and right of -1..1, a gap of nothing at the
/// top for the background.
fn quads(device: &wgpu::Device) -> Mesh {
    let white = [1.0; 4];
    let quad = |x0: f32, x1: f32| {
        [
            Vertex::new([x0, 0.5], white, [0.0, 0.0]),
            Vertex::new([x0, -1.0], white, [0.0, 1.0]),
            Vertex::new([x1, -1.0], white, [1.0, 1.0]),
            Vertex::new([x1, 0.5], white, [1.0, 0.0]),
        ]
    };
    let vertices: Vec<Vertex> = quad(-1.0, 0.0).into_iter().chain(quad(0.0, 1.0)).collect();
    Mesh::indexed(
        device,
        "layer-quads",
        &vertices,
        &[0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4],
    )
}

/// The frame lit by one lamp of `layers`, the scene's layers drawn, the
/// left quad on `left` (the right always on layer 2).
fn frame_with(layers: u32, left: u32) -> Option<Vec<Rgba>> {
    let (device, queue) = headless_device()?;
    let scene = RenderTarget::new(&device, "scene", FORMAT, SIZE, SIZE, Filter::Linear);
    let output = RenderTarget::new(&device, "output", FORMAT, SIZE, SIZE, Filter::Linear);
    let layer_target =
        RenderTarget::new(&device, "layers", LAYER_FORMAT, SIZE, SIZE, Filter::Nearest);
    let camera = CameraBinding::new(&device);
    camera.upload(&queue, &Camera2D::new(Vec2::ZERO, 1.0, 1.0));
    let layout = texture::bind_group_layout(&device);
    let sprites = pipeline::create_flat_color(&device, FORMAT, camera.layout(), &layout);
    let white = Texture::white(&device, &queue).create_bind_group(&device, &layout);
    let mesh = quads(&device);
    let range = |indices, light_layers| MeshDraw {
        texture: &white,
        normal: None,
        light_layers,
        indices,
    };
    let mesh_draws = [range(0..6, left), range(6..12, 2)];
    let draws = [
        SceneDraw::Mesh(range(0..6, left)),
        SceneDraw::Mesh(range(6..12, 2)),
    ];

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("light-layers-test"),
    });
    pass::draw_mesh_batches(
        &mut encoder,
        scene.view(),
        &sprites,
        camera.bind_group(),
        Some(&mesh),
        &mesh_draws,
        wgpu::Color::WHITE,
    );
    LayerPass::new(&device, camera.layout(), &layout).record(
        &device,
        &mut encoder,
        layer_target.view(),
        camera.bind_group(),
        Some(&mesh),
        &draws,
    );

    let lighting = Lighting {
        ambient: Some([0.25; 3]),
        lights: vec![PointLight {
            position: Vec2::ZERO,
            color: [1.0; 3],
            intensity: 2.0,
            radius: 4.0,
            falloff: 1.0,
            shadow: None,
            cone: None,
            height: None,
            layers,
        }],
        ..Lighting::default()
    };
    assert_eq!(lighting.uses_layers(), layers != ALL_LIGHT_LAYERS);
    let lights = LightPass::new(&device, camera.layout());
    let grade = PostPass::new(&device, FORMAT);
    let mut pool = TexturePool::default();
    let mut graph = RenderGraph::new();
    let drawn = graph.import(scene.view());
    let target = graph.import_output(output.view());
    let source = lights
        .add(
            &mut graph,
            &device,
            camera.bind_group(),
            drawn,
            SIZE,
            SIZE,
            &lighting,
            None,
            Some(layer_target.view()),
        )
        .unwrap_or(drawn);
    let (device_ref, queue_ref) = (&device, &queue);
    graph.add_pass("grade", &[source], &[target], move |encoder, views| {
        grade.record(
            device_ref,
            queue_ref,
            encoder,
            views.get(source),
            None,
            views.get(target),
            &PostSettings::NEUTRAL,
            1.0,
        );
    });
    graph
        .run(&device, &mut encoder, &mut pool)
        .expect("a valid graph");
    queue.submit(Some(encoder.finish()));
    Some(read_texture(&device, &queue, output.texture(), SIZE, SIZE))
}

/// [`frame_with`], the left quad on layer 1.
fn frame(layers: u32) -> Option<Vec<Rgba>> {
    frame_with(layers, 1)
}

fn at(pixels: &[Rgba], x: u32, y: u32) -> u8 {
    pixels[(y * SIZE + x) as usize].r
}

/// The ambient alone, 0.25 linear: 137 in sRGB.
const AMBIENT: u8 = 137;

#[test]
fn a_lamp_lights_only_what_shares_one_of_its_layers() {
    let Some(pixels) = frame(0b10) else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let (left, right) = (at(&pixels, 16, 40), at(&pixels, 48, 40));
    assert!(left.abs_diff(AMBIENT) <= 3, "layer 1, unlit by it: {left}");
    assert!(right > 200, "layer 2, lit: {right}");
    // Above the quads nothing was drawn: on every layer, so lit.
    assert!(at(&pixels, 32, 4) > 200, "the background");
}

#[test]
fn a_lamp_of_every_layer_lights_everything() {
    let Some(pixels) = frame(ALL_LIGHT_LAYERS) else {
        return;
    };
    assert!(at(&pixels, 16, 40) > 200 && at(&pixels, 48, 40) > 200);
}

#[test]
fn a_lamp_of_no_layer_lights_nothing() {
    let Some(pixels) = frame(0) else {
        return;
    };
    for (x, y) in [(16, 40), (48, 40), (32, 4)] {
        assert!(at(&pixels, x, y).abs_diff(AMBIENT) <= 3, "({x}, {y})");
    }
}

#[test]
fn a_surface_on_no_layer_is_lit_by_nothing_even_a_lamp_of_every_layer() {
    let Some(pixels) = frame_with(ALL_LIGHT_LAYERS, 0) else {
        return;
    };
    assert!(at(&pixels, 16, 40).abs_diff(AMBIENT) <= 3, "on no layer");
    assert!(at(&pixels, 48, 40) > 200, "on layer 2");
}
