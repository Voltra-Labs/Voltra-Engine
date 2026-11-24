//! Quads drawn as instances (stage 108): a turned, textured quad comes out
//! as the same quad drawn as two triangles does, the two kinds keep
//! painter's order between them, and buffers kept from frame to frame draw
//! what was last written into them.
//!
//! Skips itself when no GPU adapter is available.

use voltra_render::camera::{Camera2D, CameraBinding};
use voltra_render::glam::{Mat3, Vec2};
use voltra_render::pass::{self, MeshDraw};
use voltra_render::{
    pipeline, texture, wgpu, Filter, Mesh, MeshBuffers, Primitives, QuadInstance, Texture,
};
use voltra_testkit::{headless_device, read_texture, Rgba};

const SIZE: u32 = 32;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

struct Scene {
    device: wgpu::Device,
    queue: wgpu::Queue,
    target: wgpu::Texture,
    view: wgpu::TextureView,
    camera: CameraBinding,
    pipeline: voltra_render::SpritePipeline,
    /// Four texels: red, green / blue, white.
    checker: wgpu::BindGroup,
    white: wgpu::BindGroup,
}

impl Scene {
    fn new() -> Option<Self> {
        let (device, queue) = headless_device()?;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("headless-quads-target"),
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
        let pipeline = pipeline::create_flat_color(&device, FORMAT, camera.layout(), &layout);
        #[rustfmt::skip]
        let texels = [
            255, 0, 0, 255,   0, 255, 0, 255,
            0, 0, 255, 255,   255, 255, 255, 255,
        ];
        let checker =
            Texture::from_rgba8(&device, &queue, "checker", &texels, 2, 2, Filter::Nearest)
                .expect("a 2x2 texture")
                .create_bind_group(&device, &layout);
        let white = Texture::white(&device, &queue).create_bind_group(&device, &layout);
        Some(Self {
            device,
            queue,
            target,
            view,
            camera,
            pipeline,
            checker,
            white,
        })
    }

    fn draw(&self, mesh: Option<&Mesh>, primitives: &[Primitives]) -> Vec<Rgba> {
        self.draw_with(&self.checker, mesh, primitives)
    }

    fn draw_with(
        &self,
        texture: &wgpu::BindGroup,
        mesh: Option<&Mesh>,
        primitives: &[Primitives],
    ) -> Vec<Rgba> {
        let draws: Vec<MeshDraw<'_>> = primitives
            .iter()
            .map(|primitives| MeshDraw {
                texture,
                normal: None,
                light_layers: voltra_render::DEFAULT_LIGHT_LAYERS,
                primitives: primitives.clone(),
                material: None,
            })
            .collect();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("headless-quads-encoder"),
            });
        pass::draw_mesh_batches(
            &mut encoder,
            &self.view,
            &self.pipeline,
            self.camera.bind_group(),
            mesh,
            &draws,
            wgpu::Color::BLACK,
        );
        self.queue.submit(Some(encoder.finish()));
        read_texture(&self.device, &self.queue, &self.target, SIZE, SIZE)
    }
}

/// A square of half-size `half` turned by `angle` about `at`, sampling the
/// whole texture, as an instance.
fn turned(at: Vec2, half: f32, angle: f32, color: [f32; 4]) -> QuadInstance {
    let matrix = Mat3::from_scale_angle_translation(Vec2::splat(half), angle, at);
    let corners = [(-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0), (1.0, 1.0)]
        .map(|(x, y)| matrix.transform_point2(Vec2::new(x, y)).to_array());
    let uvs = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];
    QuadInstance::from_corners(corners, uvs, color).expect("a turned square")
}

fn differing(a: &[Rgba], b: &[Rgba]) -> usize {
    a.iter()
        .zip(b)
        .filter(|(a, b)| {
            [(a.r, b.r), (a.g, b.g), (a.b, b.b)]
                .iter()
                .any(|(x, y)| x.abs_diff(*y) > 2)
        })
        .count()
}

#[test]
fn an_instance_draws_what_its_two_triangles_draw() {
    let Some(scene) = Scene::new() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let quad = turned(Vec2::ZERO, 0.7, 0.4, [1.0; 4]);
    let instanced = Mesh::batch(
        &scene.device,
        "quads",
        &[] as &[voltra_render::Vertex],
        &[],
        &[quad],
    );
    let by_quads = scene.draw(Some(&instanced), &[Primitives::Quads(0..1)]);
    let triangles = Mesh::indexed(
        &scene.device,
        "triangles",
        &quad.corners(),
        &[0, 1, 2, 0, 2, 3],
    );
    let by_triangles = scene.draw(Some(&triangles), &[Primitives::Triangles(0..6)]);

    let lit = by_quads
        .iter()
        .filter(|p| p.r > 0 || p.g > 0 || p.b > 0)
        .count();
    assert!(lit > 200, "the quad covers the middle: {lit} pixels");
    // Every texel colour shows: the UVs reached the corners.
    for colour in [(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 255)] {
        assert!(
            by_quads.iter().any(|p| (p.r, p.g, p.b) == colour),
            "{colour:?} is sampled somewhere"
        );
    }
    // The same pixels, but for an edge rounded the other way here and there.
    assert!(
        differing(&by_quads, &by_triangles) <= 4,
        "{} pixels differ",
        differing(&by_quads, &by_triangles)
    );
}

#[test]
fn quads_and_triangles_keep_painters_order_between_them() {
    let Some(scene) = Scene::new() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    // A red-tinted quad over the whole target, a green triangle pair over
    // its middle, then a blue quad over a corner: drawn in that order.
    let red = turned(Vec2::ZERO, 2.0, 0.0, [1.0, 0.0, 0.0, 1.0]);
    let blue = turned(Vec2::new(-0.75, 0.75), 0.25, 0.0, [0.0, 0.0, 1.0, 1.0]);
    let green = turned(Vec2::ZERO, 0.25, 0.0, [0.0, 1.0, 0.0, 1.0]).corners();
    let mesh = Mesh::batch(
        &scene.device,
        "mixed",
        &green,
        &[0, 1, 2, 0, 2, 3],
        &[red, blue],
    );
    let pixels = scene.draw_with(
        &scene.white,
        Some(&mesh),
        &[
            Primitives::Quads(0..1),
            Primitives::Triangles(0..6),
            Primitives::Quads(1..2),
        ],
    );
    let at = |x: u32, y: u32| pixels[(y * SIZE + x) as usize];
    assert!(
        at(SIZE / 2, SIZE / 2).g > 0 && at(SIZE / 2, SIZE / 2).r == 0,
        "green over red"
    );
    assert!(at(2, 2).b > 0 && at(2, 2).r == 0, "blue over red, top left");
    assert!(at(SIZE - 3, SIZE - 3).r > 0, "red elsewhere");
}

#[test]
fn kept_buffers_grow_shrink_and_draw_the_last_write() {
    let Some(scene) = Scene::new() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let mut buffers = MeshBuffers::new("kept");
    let none: &[voltra_render::Vertex] = &[];
    assert!(
        buffers
            .write(&scene.device, &scene.queue, none, &[], &[])
            .is_none(),
        "nothing to draw is no mesh"
    );

    let many: Vec<QuadInstance> = (0..5000)
        .map(|i| turned(Vec2::splat(i as f32), 0.1, 0.0, [1.0; 4]))
        .collect();
    buffers.write(&scene.device, &scene.queue, none, &[], &many);
    let large = buffers.capacity();
    assert!(large >= (5000 * size_of::<QuadInstance>()) as u64);
    buffers.write(&scene.device, &scene.queue, none, &[], &many[..4000]);
    assert_eq!(
        buffers.capacity(),
        large,
        "a smaller frame reuses the buffer"
    );

    // One quad, far less than a quarter: the buffer is made again, small,
    // and what it draws is this frame's, not the 5000 written before.
    let only = turned(Vec2::ZERO, 0.5, 0.0, [0.0, 1.0, 0.0, 1.0]);
    let mesh = buffers
        .write(&scene.device, &scene.queue, none, &[], &[only])
        .expect("a quad to draw");
    assert_eq!(mesh.quad_count(), 1);
    let pixels = scene.draw_with(&scene.white, Some(mesh), &[Primitives::Quads(0..1)]);
    assert!(buffers.capacity() < large, "shrunk: {}", buffers.capacity());
    let middle = pixels[(SIZE / 2 * SIZE + SIZE / 2) as usize];
    assert!(middle.g > 0 && middle.r == 0, "the green quad: {middle:?}");
    let corner = pixels[0];
    assert_eq!((corner.r, corner.g, corner.b), (0, 0, 0), "nothing else");
}
