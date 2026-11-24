//! A vertex colour's alpha reaches the blend: half-transparent red over a
//! black clear comes out half-way, not solid.
//!
//! Skips itself when no GPU adapter is available.

use voltra_render::camera::{Camera2D, CameraBinding};
use voltra_render::glam::Vec2;
use voltra_render::pass::{self, MeshDraw};
use voltra_render::{pipeline, texture, wgpu, Mesh, Texture, Vertex};
use voltra_testkit::{headless_device, read_texture};

const SIZE: u32 = 8;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

#[test]
fn half_alpha_red_over_black_is_half_red() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("headless-alpha-target"),
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

    let camera = CameraBinding::new(&device);
    camera.upload(&queue, &Camera2D::new(Vec2::ZERO, 1.0, 1.0));
    let layout = texture::bind_group_layout(&device);
    let white = Texture::white(&device, &queue).create_bind_group(&device, &layout);
    let pipeline = pipeline::create_flat_color(&device, FORMAT, camera.layout(), &layout);

    let red = [1.0, 0.0, 0.0, 0.5];
    let vertices = [
        Vertex::new([-1.0, 1.0], red, [0.0, 0.0]),
        Vertex::new([-1.0, -1.0], red, [0.0, 1.0]),
        Vertex::new([1.0, -1.0], red, [1.0, 1.0]),
        Vertex::new([1.0, 1.0], red, [1.0, 0.0]),
    ];
    let mesh = Mesh::indexed(&device, "alpha-quad", &vertices, &[0, 1, 2, 0, 2, 3]);
    let draws = [MeshDraw {
        texture: &white,
        normal: None,
        light_layers: voltra_render::DEFAULT_LIGHT_LAYERS,
        indices: 0..6,
        material: None,
    }];

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("headless-alpha-encoder"),
    });
    pass::draw_mesh_batches(
        &mut encoder,
        &view,
        &pipeline,
        camera.bind_group(),
        Some(&mesh),
        &draws,
        wgpu::Color::BLACK,
    );
    queue.submit(Some(encoder.finish()));

    let pixel = read_texture(&device, &queue, &target, SIZE, SIZE)[(4 * SIZE + 4) as usize];
    // 0.5 linear encodes to about 188 in sRGB; solid red would be 255.
    assert!(
        (170..=205).contains(&pixel.r),
        "half-way red, not solid: {pixel:?}"
    );
    assert!(pixel.g < 10 && pixel.b < 10, "{pixel:?}");
}
