//! The post pass in pixels: neutral settings leave the frame as it was, a
//! vignette darkens the corners and not the centre, and saturation 0 is grey.
//!
//! Skips itself when no GPU adapter is available.

use voltra_render::{wgpu, Filter, PostPass, PostSettings, RenderTarget};
use voltra_testkit::{headless_device, read_texture, Rgba};

const SIZE: u32 = 32;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Clears a source target to `colour` (linear), grades it into an output
/// target with `settings`, and reads the output back.
fn graded(colour: wgpu::Color, settings: PostSettings) -> Option<Vec<Rgba>> {
    let (device, queue) = headless_device()?;
    let source = RenderTarget::new(&device, "post-source", FORMAT, SIZE, SIZE, Filter::Linear);
    let output = RenderTarget::new(&device, "post-output", FORMAT, SIZE, SIZE, Filter::Linear);
    let post = PostPass::new(&device, FORMAT);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("post-test"),
    });
    {
        // A cleared pass is the source's whole content.
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("fill-source"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: source.view(),
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(colour),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    post.record(
        &device,
        &queue,
        &mut encoder,
        source.view(),
        None,
        output.view(),
        &settings,
        1.0,
    );
    queue.submit(Some(encoder.finish()));
    Some(read_texture(&device, &queue, output.texture(), SIZE, SIZE))
}

fn at(pixels: &[Rgba], x: u32, y: u32) -> Rgba {
    pixels[(y * SIZE + x) as usize]
}

const GREY: wgpu::Color = wgpu::Color {
    r: 0.5,
    g: 0.5,
    b: 0.5,
    a: 1.0,
};

#[test]
fn neutral_settings_leave_the_frame_as_it_was() {
    let Some(pixels) = graded(GREY, PostSettings::NEUTRAL) else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    // 0.5 linear is 188 in sRGB; a double conversion would land far lower.
    for pixel in [at(&pixels, 0, 0), at(&pixels, 16, 16), at(&pixels, 31, 31)] {
        assert!((186..=190).contains(&pixel.r), "{pixel:?}");
        assert_eq!(
            (pixel.r, pixel.g),
            (pixel.g, pixel.b),
            "still grey: {pixel:?}"
        );
    }
}

#[test]
fn a_vignette_darkens_the_corners_and_not_the_centre() {
    let Some(pixels) = graded(
        GREY,
        PostSettings {
            vignette: 1.0,
            ..PostSettings::NEUTRAL
        },
    ) else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let centre = at(&pixels, 16, 16);
    let corner = at(&pixels, 0, 0);
    assert!(
        (186..=190).contains(&centre.r),
        "centre untouched: {centre:?}"
    );
    assert!(corner.r < 120, "corner darkened: {corner:?}");
}

#[test]
fn saturation_zero_is_grey_and_exposure_scales() {
    let red = wgpu::Color {
        r: 0.5,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let Some(pixels) = graded(
        red,
        PostSettings {
            saturation: 0.0,
            ..PostSettings::NEUTRAL
        },
    ) else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let p = at(&pixels, 16, 16);
    assert!(p.r == p.g && p.g == p.b, "greyscale: {p:?}");

    let Some(brighter) = graded(
        GREY,
        PostSettings {
            exposure: 2.0,
            ..PostSettings::NEUTRAL
        },
    ) else {
        return;
    };
    assert!(at(&brighter, 16, 16).r > 240, "{:?}", at(&brighter, 16, 16));
}

/// The editor's frame, pass for pass: the world into one target, graded into
/// a second, and an ungraded layer drawn over the second — the sequence
/// `Renderer::render_scene` records, with the same pipelines and formats.
#[test]
fn the_hud_drawn_after_the_grade_is_not_graded() {
    use voltra_render::camera::{screen_projection, Camera2D, CameraBinding};
    use voltra_render::glam::Vec2;
    use voltra_render::pass::{self, MeshDraw};
    use voltra_render::{pipeline, texture, Mesh, Texture, Vertex};

    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let scene = RenderTarget::new(&device, "scene", FORMAT, SIZE, SIZE, Filter::Linear);
    let graded = RenderTarget::new(&device, "graded", FORMAT, SIZE, SIZE, Filter::Linear);
    let post = PostPass::new(&device, FORMAT);
    let world_camera = CameraBinding::new(&device);
    world_camera.upload(&queue, &Camera2D::new(Vec2::ZERO, 1.0, 1.0));
    let ui_camera = world_camera.sharing_layout(&device);
    ui_camera.upload_matrix(&queue, screen_projection(SIZE as f32, SIZE as f32));
    let layout = texture::bind_group_layout(&device);
    let white = Texture::white(&device, &queue).create_bind_group(&device, &layout);
    let sprites = pipeline::create_flat_color(&device, FORMAT, world_camera.layout(), &layout);

    // A red HUD square over the top-left quarter, in pixels.
    let red = [1.0, 0.0, 0.0, 1.0];
    let half = SIZE as f32 / 2.0;
    let hud = [
        Vertex::new([0.0, 0.0], red, [0.0, 0.0]),
        Vertex::new([0.0, half], red, [0.0, 1.0]),
        Vertex::new([half, half], red, [1.0, 1.0]),
        Vertex::new([half, 0.0], red, [1.0, 0.0]),
    ];
    let hud_mesh = Mesh::indexed(&device, "hud", &hud, &[0, 1, 2, 0, 2, 3]);
    let draws = [MeshDraw {
        texture: &white,
        normal: None,
        light_layers: voltra_render::DEFAULT_LIGHT_LAYERS,
        indices: 0..6,
        material: None,
    }];

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("editor-frame"),
    });
    // The world: an empty scene is its clear colour, mid grey.
    pass::draw_mesh_batches(
        &mut encoder,
        scene.view(),
        &sprites,
        world_camera.bind_group(),
        None,
        &[],
        GREY,
    );
    // Graded to greyscale-and-dark, so an ungraded red is unmistakable.
    let grade = PostSettings {
        saturation: 0.0,
        exposure: 0.25,
        ..PostSettings::NEUTRAL
    };
    post.record(
        &device,
        &queue,
        &mut encoder,
        scene.view(),
        None,
        graded.view(),
        &grade,
        1.0,
    );
    pass::draw_mesh_batches_over(
        &mut encoder,
        graded.view(),
        &sprites,
        ui_camera.bind_group(),
        Some(&hud_mesh),
        &draws,
    );
    queue.submit(Some(encoder.finish()));
    let pixels = read_texture(&device, &queue, graded.texture(), SIZE, SIZE);

    let hud_pixel = at(&pixels, 4, 4);
    assert!(
        hud_pixel.r > 240 && hud_pixel.g < 10,
        "the HUD is not graded: {hud_pixel:?}"
    );
    let world_pixel = at(&pixels, 24, 24);
    assert!(world_pixel.r < 150, "the world is: {world_pixel:?}");
}
