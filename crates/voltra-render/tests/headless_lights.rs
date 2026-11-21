//! 2D lights in pixels, through the graph the renderer builds: a white
//! frame lit by a dim ambient and one lamp is the ambient's grey far from
//! the lamp and bright at it, fading between; unlit, the frame is as drawn
//! and no lighting texture is made.
//!
//! Skips itself when no GPU adapter is available.

use voltra_render::camera::{Camera2D, CameraBinding};
use voltra_render::glam::Vec2;
use voltra_render::graph::{RenderGraph, TexturePool};
use voltra_render::{
    pass, wgpu, Filter, LightPass, Lighting, PointLight, PostPass, PostSettings, RenderTarget,
};
use voltra_testkit::{headless_device, read_texture, Rgba};

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// A white frame — the camera sees -1..1, 32 pixels a unit — lit by `lighting`, and how
/// many textures the graph's pool ended with.
fn frame(lighting: &Lighting) -> Option<(Vec<Rgba>, usize)> {
    let (device, queue) = headless_device()?;
    let scene = RenderTarget::new(&device, "scene", FORMAT, SIZE, SIZE, Filter::Linear);
    let output = RenderTarget::new(&device, "output", FORMAT, SIZE, SIZE, Filter::Linear);
    let camera = CameraBinding::new(&device);
    camera.upload(&queue, &Camera2D::new(Vec2::ZERO, 1.0, 1.0));
    let sprites = voltra_render::pipeline::create_flat_color(
        &device,
        FORMAT,
        camera.layout(),
        &voltra_render::texture::bind_group_layout(&device),
    );
    let lights = LightPass::new(&device, camera.layout());
    let grade = PostPass::new(&device, FORMAT);
    let mut pool = TexturePool::default();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("lights-test"),
    });
    // Nothing drawn: the clear is the white frame.
    pass::draw_mesh_batches(
        &mut encoder,
        scene.view(),
        &sprites,
        camera.bind_group(),
        None,
        &[],
        wgpu::Color::WHITE,
    );

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
            lighting,
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
    Some((
        read_texture(&device, &queue, output.texture(), SIZE, SIZE),
        pool.len(),
    ))
}

fn at(pixels: &[Rgba], x: u32, y: u32) -> Rgba {
    pixels[(y * SIZE + x) as usize]
}

/// A quarter-grey ambient: 0.25 linear, 137 in sRGB.
fn dim() -> Lighting {
    Lighting {
        ambient: Some([0.25; 3]),
        lights: Vec::new(),
    }
}

#[test]
fn a_lamp_lights_its_disc_and_the_ambient_lights_the_rest() {
    let lamp = PointLight {
        position: Vec2::ZERO,
        color: [1.0; 3],
        intensity: 1.0,
        radius: 0.25,
        falloff: 1.0,
    };
    let Some((pixels, pool)) = frame(&Lighting {
        lights: vec![lamp],
        ..dim()
    }) else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    assert_eq!(pool, 2, "a light map and a lit frame");
    let (centre, between, corner) = (at(&pixels, 32, 32), at(&pixels, 27, 32), at(&pixels, 1, 1));
    assert!(corner.r.abs_diff(137) <= 3, "the ambient alone: {corner:?}");
    assert!(centre.r > 250, "lit past white at the lamp: {centre:?}");
    assert!(
        corner.r < between.r && between.r < centre.r,
        "fading from the lamp: {corner:?} {between:?} {centre:?}"
    );
    // Outside the radius (8 px from the centre) only the ambient is left.
    assert!(at(&pixels, 32, 10).r.abs_diff(137) <= 3);
}

#[test]
fn an_unlit_frame_is_as_drawn_and_makes_no_texture() {
    let Some((pixels, pool)) = frame(&Lighting::default()) else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    assert_eq!(pool, 0);
    assert_eq!(at(&pixels, 1, 1).r, 255);
}

#[test]
fn a_light_that_gives_nothing_leaves_the_ambient() {
    let dark = PointLight {
        position: Vec2::ZERO,
        color: [1.0; 3],
        intensity: 1.0,
        radius: 0.0,
        falloff: 2.0,
    };
    let Some((pixels, _)) = frame(&Lighting {
        lights: vec![dark],
        ..dim()
    }) else {
        return;
    };
    assert_eq!(at(&pixels, 32, 32), at(&pixels, 1, 1));
}
