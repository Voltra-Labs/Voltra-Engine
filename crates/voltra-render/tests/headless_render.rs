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
use voltra_render::mesh::{self, Mesh, Vertex};
use voltra_render::pass::MeshDraw;
use voltra_render::texture::{self, Filter, Texture};
use voltra_render::{pass, pipeline, wgpu};
use voltra_testkit::{headless_device, read_texture, Rgba, CLEAR};

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Draws `mesh` through `camera` into a `SIZE`x`SIZE` texture and reads the
/// pixels back.
fn render_to_pixels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mesh: &Mesh,
    camera: &Camera2D,
) -> Vec<Rgba> {
    render_textured(device, queue, mesh, camera, &Texture::white(device, queue))
}

/// As [`render_to_pixels`], but with an explicit texture bound to group 1.
fn render_textured(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mesh: &Mesh,
    camera: &Camera2D,
    albedo: &Texture,
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

    let camera_binding = CameraBinding::new(device);
    camera_binding.upload(queue, camera);
    let texture_layout = texture::bind_group_layout(device);
    let texture_bind_group = albedo.create_bind_group(device, &texture_layout);
    let render_pipeline =
        pipeline::create_flat_color(device, FORMAT, camera_binding.layout(), &texture_layout);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("headless-encoder"),
    });
    pass::draw_mesh(
        &mut encoder,
        &view,
        &render_pipeline,
        camera_binding.bind_group(),
        &texture_bind_group,
        Some(mesh),
        CLEAR,
    );
    queue.submit(Some(encoder.finish()));

    read_texture(device, queue, &texture, SIZE, SIZE)
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
    let survivors = pixels.iter().filter(|p| p.is_clear_ish()).count();
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

/// A full-screen quad with white vertex colours, so the texture is the only
/// thing deciding the output.
const WHITE_QUAD: [voltra_render::Vertex; 4] = [
    voltra_render::Vertex::new([-1.0, 1.0], [1.0, 1.0, 1.0], [0.0, 0.0]),
    voltra_render::Vertex::new([-1.0, -1.0], [1.0, 1.0, 1.0], [0.0, 1.0]),
    voltra_render::Vertex::new([1.0, -1.0], [1.0, 1.0, 1.0], [1.0, 1.0]),
    voltra_render::Vertex::new([1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 0.0]),
];

#[test]
fn texture_is_sampled_the_right_way_up() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter available; skipping headless render test");
        return;
    };

    // 2x2, one solid colour per texel, laid out row-major from the top left.
    #[rustfmt::skip]
    let texels: [u8; 16] = [
        255, 0, 0, 255,     0, 255, 0, 255,
        0, 0, 255, 255,     255, 255, 0, 255,
    ];
    let checker = Texture::from_rgba8(&device, &queue, "checker", &texels, 2, 2, Filter::Nearest)
        .expect("2x2 RGBA is 16 bytes");

    let quad = Mesh::indexed(&device, "white-quad", &WHITE_QUAD, &mesh::QUAD_INDICES);
    let pixels = render_textured(&device, &queue, &quad, &Camera2D::default(), &checker);

    // Sample well inside each quadrant to stay clear of the texel boundary.
    let q = SIZE / 4;
    let top_left = at(&pixels, q, q);
    let top_right = at(&pixels, SIZE - q, q);
    let bottom_left = at(&pixels, q, SIZE - q);
    let bottom_right = at(&pixels, SIZE - q, SIZE - q);

    // If V were flipped, top and bottom would swap and every one of these
    // would fail — which is the point of checking all four.
    assert!(
        top_left.r > 200 && top_left.g < 60 && top_left.b < 60,
        "top-left should be red, got {top_left:?}"
    );
    assert!(
        top_right.g > 200 && top_right.r < 60 && top_right.b < 60,
        "top-right should be green, got {top_right:?}"
    );
    assert!(
        bottom_left.b > 200 && bottom_left.r < 60 && bottom_left.g < 60,
        "bottom-left should be blue, got {bottom_left:?}"
    );
    assert!(
        bottom_right.r > 200 && bottom_right.g > 200 && bottom_right.b < 60,
        "bottom-right should be yellow, got {bottom_right:?}"
    );
}

#[test]
fn white_texture_leaves_vertex_colour_alone() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter available; skipping headless render test");
        return;
    };

    let white = Texture::white(&device, &queue);
    assert_eq!((white.width(), white.height()), (1, 1));

    let triangle = Mesh::new(&device, "test-triangle", &mesh::TRIANGLE);
    let textured = render_textured(
        &device,
        &queue,
        &triangle,
        &Camera2D::default(),
        &Texture::white(&device, &queue),
    );

    // Multiplying by white is a no-op, so this must match what the untextured
    // path produced. That equivalence is what allows a single pipeline.
    let centre = at(&textured, SIZE / 2, SIZE / 2);
    assert!(
        centre.r > centre.g && centre.r > centre.b && centre.r > 150,
        "white texture should not tint the vertex colours, got {centre:?}"
    );
}

#[test]
fn wrong_sized_pixel_data_is_rejected() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter available; skipping headless render test");
        return;
    };

    // 2x2 RGBA needs 16 bytes; hand it 12.
    let err = Texture::from_rgba8(&device, &queue, "short", &[0; 12], 2, 2, Filter::Nearest)
        .expect_err("short pixel data must not be accepted");

    // Left unchecked this reaches wgpu as a validation panic rather than a
    // Result the caller can act on.
    let text = err.to_string();
    assert!(text.contains("12") && text.contains("16"), "{text}");
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

/// Two side-by-side quads sharing one vertex/index buffer: x in `[-1, 0]` for
/// the first, `[0, 1]` for the second. Each is its own indexed range so
/// [`pass::draw_mesh_batches`] can bind a different texture per half.
const TWO_HALVES: [Vertex; 8] = [
    Vertex::new([-1.0, 1.0], [1.0, 1.0, 1.0], [0.0, 0.0]),
    Vertex::new([-1.0, -1.0], [1.0, 1.0, 1.0], [0.0, 1.0]),
    Vertex::new([0.0, -1.0], [1.0, 1.0, 1.0], [1.0, 1.0]),
    Vertex::new([0.0, 1.0], [1.0, 1.0, 1.0], [1.0, 0.0]),
    Vertex::new([0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 0.0]),
    Vertex::new([0.0, -1.0], [1.0, 1.0, 1.0], [0.0, 1.0]),
    Vertex::new([1.0, -1.0], [1.0, 1.0, 1.0], [1.0, 1.0]),
    Vertex::new([1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 0.0]),
];
const TWO_HALVES_INDICES: [u32; 12] = [0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];

#[test]
fn draw_mesh_batches_binds_a_different_texture_per_range() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter available; skipping headless render test");
        return;
    };

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

    let camera_binding = CameraBinding::new(&device);
    camera_binding.upload(&queue, &Camera2D::default());
    let texture_layout = texture::bind_group_layout(&device);
    let render_pipeline =
        pipeline::create_flat_color(&device, FORMAT, camera_binding.layout(), &texture_layout);

    let white = Texture::white(&device, &queue).create_bind_group(&device, &texture_layout);
    let red = Texture::from_rgba8(
        &device,
        &queue,
        "red",
        &[255, 0, 0, 255],
        1,
        1,
        Filter::Nearest,
    )
    .expect("1x1 RGBA is 4 bytes")
    .create_bind_group(&device, &texture_layout);

    let mesh = Mesh::indexed(&device, "two-halves", &TWO_HALVES, &TWO_HALVES_INDICES);
    let draws = [
        MeshDraw {
            texture: &white,
            indices: 0..6,
        },
        MeshDraw {
            texture: &red,
            indices: 6..12,
        },
    ];

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("headless-encoder"),
    });
    pass::draw_mesh_batches(
        &mut encoder,
        &view,
        &render_pipeline,
        camera_binding.bind_group(),
        Some(&mesh),
        &draws,
        CLEAR,
    );
    queue.submit(Some(encoder.finish()));

    let pixels = read_texture(&device, &queue, &texture, SIZE, SIZE);

    // Left half went through the white bind group, a no-op on the white
    // vertex colour; right half went through the red one.
    let left = at(&pixels, SIZE / 4, SIZE / 2);
    let right = at(&pixels, 3 * SIZE / 4, SIZE / 2);
    assert!(
        left.r > 200 && left.g > 200 && left.b > 200,
        "left half should stay white, got {left:?}"
    );
    assert!(
        right.r > 200 && right.g < 60 && right.b < 60,
        "right half should be tinted red, got {right:?}"
    );
}

#[test]
fn draw_mesh_batches_with_no_draws_only_clears() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter available; skipping headless render test");
        return;
    };

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

    let camera_binding = CameraBinding::new(&device);
    camera_binding.upload(&queue, &Camera2D::default());
    let texture_layout = texture::bind_group_layout(&device);
    let render_pipeline =
        pipeline::create_flat_color(&device, FORMAT, camera_binding.layout(), &texture_layout);
    let mesh = Mesh::indexed(&device, "two-halves", &TWO_HALVES, &TWO_HALVES_INDICES);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("headless-encoder"),
    });
    pass::draw_mesh_batches(
        &mut encoder,
        &view,
        &render_pipeline,
        camera_binding.bind_group(),
        Some(&mesh),
        &[],
        CLEAR,
    );
    queue.submit(Some(encoder.finish()));

    let pixels = read_texture(&device, &queue, &texture, SIZE, SIZE);
    let survivors = pixels.iter().filter(|p| p.is_clear_ish()).count();
    assert_eq!(
        survivors,
        pixels.len(),
        "empty draws must leave nothing but the clear colour"
    );
}
