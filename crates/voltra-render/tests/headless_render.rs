//! Renders the built-in meshes into an offscreen texture and inspects the
//! resulting pixels.
//!
//! This is the only honest way to prove the pipeline actually rasterises
//! something. A pipeline can be built, bound and drawn with zero validation
//! errors and still produce an empty frame — culling, a degenerate winding, a
//! bad viewport or a silently-failed shader all look identical from the API
//! side. Nothing short of reading the pixels back distinguishes them, and
//! eyeballing a window proves nothing you can put in CI.
//!
//! The test skips itself when no GPU adapter is available so CI machines
//! without one still pass.

use voltra_render::camera::{Camera2D, CameraBinding};
use voltra_render::glam::Vec2;
use voltra_render::mesh::{self, Mesh};
use voltra_render::{pass, pipeline, wgpu};

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.1,
    g: 0.1,
    b: 0.12,
    a: 1.0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

/// Returns `None` when the machine has no usable adapter.
fn headless_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());

    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;

    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("headless-test-device"),
        ..Default::default()
    }))
    .ok()
}

/// Draws `mesh` through `camera` into a `SIZE`x`SIZE` texture and reads the
/// pixels back.
fn render_to_pixels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mesh: &Mesh,
    camera: &Camera2D,
) -> Vec<Rgba> {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("headless-target"),
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
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // `copy_texture_to_buffer` requires bytes_per_row to be a multiple of 256.
    // At 64px * 4 bytes that is exactly 256, but compute it rather than rely on
    // the coincidence surviving a change to SIZE.
    let unpadded = SIZE * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded = unpadded.div_ceil(align) * align;

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("headless-readback"),
        size: (padded * SIZE) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let camera_binding = CameraBinding::new(device);
    camera_binding.upload(queue, camera);
    let render_pipeline = pipeline::create_flat_color(device, FORMAT, camera_binding.layout());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("headless-encoder"),
    });
    pass::draw_mesh(
        &mut encoder,
        &view,
        &render_pipeline,
        camera_binding.bind_group(),
        mesh,
        CLEAR,
    );
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(SIZE),
            },
        },
        wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    readback.map_async(wgpu::MapMode::Read, .., |result| {
        result.expect("readback buffer failed to map");
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("device poll failed");

    let pixels = {
        let mapped = readback
            .get_mapped_range(..)
            .expect("readback buffer range not mapped");
        let mut out = Vec::with_capacity((SIZE * SIZE) as usize);
        for y in 0..SIZE {
            let row = (y * padded) as usize;
            for x in 0..SIZE {
                let i = row + (x * 4) as usize;
                out.push(Rgba {
                    r: mapped[i],
                    g: mapped[i + 1],
                    b: mapped[i + 2],
                    a: mapped[i + 3],
                });
            }
        }
        out
    };
    readback.unmap();

    pixels
}

fn at(pixels: &[Rgba], x: u32, y: u32) -> Rgba {
    pixels[(y * SIZE + x) as usize]
}

#[test]
fn triangle_rasterises_and_interpolates() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter available; skipping headless render test");
        return;
    };

    let triangle = Mesh::new(&device, "test-triangle", &mesh::TRIANGLE);
    let pixels = render_to_pixels(&device, &queue, &triangle, &Camera2D::default());

    // The top-left corner is outside the triangle, so it must still hold the
    // clear colour. 0.1 linear encodes to roughly 89 in an sRGB texture.
    let corner = at(&pixels, 1, 1);
    assert!(
        corner.r < 120 && corner.g < 120 && corner.b < 130,
        "corner should be the clear colour, got {corner:?}"
    );

    // Dead centre sits inside the triangle, weighted towards the red vertex
    // (barycentric 0.5 red / 0.25 green / 0.25 blue), so red must dominate.
    let centre = at(&pixels, SIZE / 2, SIZE / 2);
    assert!(
        centre.r > centre.g && centre.r > centre.b,
        "centre should be red-dominant, got {centre:?}"
    );
    assert!(
        centre.r > 150,
        "centre red channel should be bright, got {centre:?}"
    );
    assert_ne!(
        centre, corner,
        "centre and corner identical: nothing was drawn"
    );

    // Everything is opaque.
    assert_eq!(centre.a, 255);
    assert_eq!(corner.a, 255);
}

#[test]
fn indexed_quad_covers_every_pixel() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter available; skipping headless render test");
        return;
    };

    let quad = Mesh::indexed(&device, "test-quad", &mesh::QUAD, &mesh::QUAD_INDICES);
    assert!(quad.is_indexed());
    assert_eq!(quad.count(), mesh::QUAD_INDICES.len() as u32);

    let pixels = render_to_pixels(&device, &queue, &quad, &Camera2D::default());

    // The quad spans the whole of clip space, so the clear colour must be
    // completely painted over. If the index buffer were ignored or mis-typed,
    // part of the surface would survive.
    let clear_ish = |p: &Rgba| p.r < 120 && p.g < 120 && p.b < 130;
    let survivors = pixels.iter().filter(|p| clear_ish(p)).count();
    assert_eq!(
        survivors, 0,
        "{survivors} pixels still hold the clear colour"
    );

    // Corners carry the vertex colours, so opposite corners must differ —
    // that only holds if the indices mapped to the vertices we expect.
    let top_left = at(&pixels, 0, 0);
    let bottom_right = at(&pixels, SIZE - 1, SIZE - 1);
    assert_ne!(top_left, bottom_right);
    assert!(
        top_left.r > top_left.g && top_left.r > top_left.b,
        "top-left should follow the red vertex, got {top_left:?}"
    );
}

#[test]
fn moving_the_camera_moves_the_geometry() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter available; skipping headless render test");
        return;
    };

    let triangle = Mesh::new(&device, "test-triangle", &mesh::TRIANGLE);

    let centred = render_to_pixels(&device, &queue, &triangle, &Camera2D::default());
    // Pushing the camera two units right puts the triangle entirely off the
    // left edge of a viewport that only spans [-1, 1].
    let panned = render_to_pixels(
        &device,
        &queue,
        &triangle,
        &Camera2D::new(Vec2::new(2.0, 0.0), 1.0, 1.0),
    );

    let centre_before = at(&centred, SIZE / 2, SIZE / 2);
    let centre_after = at(&panned, SIZE / 2, SIZE / 2);

    assert!(
        centre_before.r > centre_before.g,
        "sanity: the unpanned triangle should still be there, got {centre_before:?}"
    );
    // If the uniform never reached the shader, both frames would be identical.
    assert_ne!(
        centre_before, centre_after,
        "camera had no effect: the uniform is not reaching the shader"
    );
    assert!(
        centre_after.r < 120 && centre_after.g < 120 && centre_after.b < 130,
        "panned frame should be pure clear colour, got {centre_after:?}"
    );
}

#[test]
fn zooming_out_shrinks_the_geometry() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter available; skipping headless render test");
        return;
    };

    let quad = Mesh::indexed(&device, "test-quad", &mesh::QUAD, &mesh::QUAD_INDICES);

    // At zoom 0.5 the viewport covers four units, so the two-unit quad only
    // fills the middle and the clear colour survives around it.
    let pixels = render_to_pixels(&device, &queue, &quad, &Camera2D::new(Vec2::ZERO, 0.5, 1.0));

    let corner = at(&pixels, 1, 1);
    let centre = at(&pixels, SIZE / 2, SIZE / 2);
    assert!(
        corner.r < 120 && corner.g < 120 && corner.b < 130,
        "zoomed out, the corner should be clear colour, got {corner:?}"
    );
    assert_ne!(centre, corner, "the quad should still cover the centre");
}
