//! GPU particles in buffers and pixels: a step births the slots it is told
//! to and moves the rest, the dead stay dead, and a draw puts a tinted quad
//! where a particle is, in painter's order with the sprites around it.
//!
//! Skips itself when no GPU adapter is available.

use voltra_render::camera::{Camera2D, CameraBinding};
use voltra_render::glam::Vec2;
use voltra_render::particles::PARTICLE_SIZE;
use voltra_render::scene_draw::{draw_scene, SceneDraw};
use voltra_render::{
    pipeline, texture, wgpu, Filter, GpuParticles, LayerPass, Mesh, MeshDraw, ParticleLook,
    ParticlePipelines, RenderTarget, SimStep, Texture, Vertex,
};
use voltra_testkit::{headless_device, read_buffer, read_texture};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// One slot as the shader lays it out.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Slot {
    position: Vec2,
    velocity: Vec2,
    age: f32,
    lifetime: f32,
}

fn slots(device: &wgpu::Device, queue: &wgpu::Queue, particles: &GpuParticles) -> Vec<Slot> {
    let size = u64::from(particles.capacity()) * PARTICLE_SIZE;
    let bytes = read_buffer(device, queue, particles.buffer(), size);
    let floats: &[f32] = bytemuck::cast_slice(&bytes);
    floats
        .chunks(8)
        .map(|f| Slot {
            position: Vec2::new(f[0], f[1]),
            velocity: Vec2::new(f[2], f[3]),
            age: f[4],
            lifetime: f[5],
        })
        .collect()
}

fn pipelines(device: &wgpu::Device) -> (ParticlePipelines, CameraBinding, wgpu::BindGroupLayout) {
    let camera = CameraBinding::new(device);
    let layout = texture::bind_group_layout(device);
    let pipelines = ParticlePipelines::new(device, FORMAT, camera.layout(), &layout);
    (pipelines, camera, layout)
}

fn step(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipelines: &ParticlePipelines,
    particles: &GpuParticles,
    step: &SimStep,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("particles-test"),
    });
    particles.simulate(queue, &mut encoder, pipelines, step);
    queue.submit(Some(encoder.finish()));
}

/// Four born at (1, 2) going right at 2 units a second, living a second.
fn fountain() -> SimStep {
    SimStep {
        dt: 0.1,
        spawn_start: 0,
        spawn_count: 4,
        seed: 1,
        origin: Vec2::new(1.0, 2.0),
        speed: (2.0, 2.0),
        lifetime: (1.0, 1.0),
        ..SimStep::default()
    }
}

#[test]
fn a_step_births_its_slots_and_the_next_moves_them() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let (pipelines, _, _) = pipelines(&device);
    let particles = GpuParticles::new(&device, &pipelines, 8);

    step(&device, &queue, &pipelines, &particles, &fountain());
    let born = slots(&device, &queue, &particles);
    for slot in &born[..4] {
        assert_eq!(slot.position, Vec2::new(1.0, 2.0), "born where it starts");
        assert!(
            (slot.velocity - Vec2::new(2.0, 0.0)).length() < 1e-5,
            "{slot:?}"
        );
        assert_eq!((slot.age, slot.lifetime), (0.0, 1.0));
    }
    for slot in &born[4..] {
        assert_eq!(slot.lifetime, 0.0, "never born: dead");
    }

    step(
        &device,
        &queue,
        &pipelines,
        &particles,
        &SimStep {
            dt: 0.5,
            acceleration: Vec2::new(0.0, -2.0),
            ..SimStep::default()
        },
    );
    let moved = slots(&device, &queue, &particles);
    let p = moved[0];
    assert!((p.age - 0.5).abs() < 1e-5);
    // Velocity first, then position — the CPU path's order.
    assert!((p.velocity - Vec2::new(2.0, -1.0)).length() < 1e-5, "{p:?}");
    assert!((p.position - Vec2::new(2.0, 1.5)).length() < 1e-5, "{p:?}");
    assert_eq!(moved[5], born[5], "the dead do not move");
}

#[test]
fn births_wrap_round_the_ring_and_spread_varies_them() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let (pipelines, _, _) = pipelines(&device);
    let particles = GpuParticles::new(&device, &pipelines, 8);
    step(
        &device,
        &queue,
        &pipelines,
        &particles,
        &SimStep {
            spawn_start: 6,
            spawn_count: 4,
            spread: std::f32::consts::TAU,
            ..fountain()
        },
    );
    let slots = slots(&device, &queue, &particles);
    let alive: Vec<usize> = (0..8).filter(|&i| slots[i].lifetime > 0.0).collect();
    assert_eq!(alive, [0, 1, 6, 7]);
    let directions: Vec<Vec2> = alive.iter().map(|&i| slots[i].velocity).collect();
    assert!(
        directions.windows(2).all(|w| (w[0] - w[1]).length() > 1e-3),
        "a full spread sends each a different way: {directions:?}"
    );
}

/// A sprite square behind the particles and one in front: the particles
/// land between them in painter's order.
#[test]
fn particles_draw_between_the_sprites_around_them() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let size = 32;
    let target = RenderTarget::new(
        &device,
        "particles-target",
        FORMAT,
        size,
        size,
        Filter::Linear,
    );
    let (pipelines, camera, layout) = pipelines(&device);
    camera.upload(&queue, &Camera2D::new(Vec2::ZERO, 1.0, 1.0));
    let white = Texture::white(&device, &queue).create_bind_group(&device, &layout);
    let sprites = pipeline::create_flat_color(&device, FORMAT, camera.layout(), &layout);

    // Blue fills the left half behind; green covers the right half in front.
    let quad = |x0: f32, x1: f32, color: [f32; 4]| {
        [
            Vertex::new([x0, 1.0], color, [0.0, 0.0]),
            Vertex::new([x0, -1.0], color, [0.0, 1.0]),
            Vertex::new([x1, -1.0], color, [1.0, 1.0]),
            Vertex::new([x1, 1.0], color, [1.0, 0.0]),
        ]
    };
    let mut vertices = quad(-1.0, 0.0, [0.0, 0.0, 1.0, 1.0]).to_vec();
    vertices.extend(quad(0.0, 1.0, [0.0, 1.0, 0.0, 1.0]));
    let mesh = Mesh::indexed(
        &device,
        "sprites",
        &vertices,
        &[0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7],
    );

    // One big red particle over the middle.
    let particles = GpuParticles::new(&device, &pipelines, 4);
    step(
        &device,
        &queue,
        &pipelines,
        &particles,
        &SimStep {
            spawn_count: 1,
            origin: Vec2::ZERO,
            lifetime: (10.0, 10.0),
            ..SimStep::default()
        },
    );
    particles.set_look(
        &queue,
        &ParticleLook {
            size: (1.0, 1.0),
            color_start: [1.0, 0.0, 0.0, 1.0],
            color_end: [1.0, 0.0, 0.0, 1.0],
            ..ParticleLook::default()
        },
    );

    let draws = [
        SceneDraw::Mesh(MeshDraw {
            texture: &white,
            normal: None,
            light_layers: voltra_render::DEFAULT_LIGHT_LAYERS,
            indices: 0..6,
        }),
        SceneDraw::Particles {
            particles: &particles,
            texture: &white,
            light_layers: voltra_render::DEFAULT_LIGHT_LAYERS,
        },
        SceneDraw::Mesh(MeshDraw {
            texture: &white,
            normal: None,
            light_layers: voltra_render::DEFAULT_LIGHT_LAYERS,
            indices: 6..12,
        }),
    ];
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("particles-frame"),
    });
    draw_scene(
        &mut encoder,
        target.view(),
        &sprites,
        Some(&pipelines),
        camera.bind_group(),
        Some(&mesh),
        &draws,
        wgpu::Color::BLACK,
    );
    queue.submit(Some(encoder.finish()));
    let pixels = read_texture(&device, &queue, target.texture(), size, size);
    let at = |x: u32, y: u32| pixels[(y * size + x) as usize];

    let left_of_centre = at(13, 16);
    assert!(
        left_of_centre.r > 200 && left_of_centre.b < 50,
        "particle over blue: {left_of_centre:?}"
    );
    let right_of_centre = at(18, 16);
    assert!(
        right_of_centre.g > 200 && right_of_centre.r < 50,
        "green over particle: {right_of_centre:?}"
    );
    let far_left = at(2, 16);
    assert!(
        far_left.b > 200 && far_left.r < 50,
        "blue where no particle is: {far_left:?}"
    );
}

/// A GPU emitter is drawn into the light-layer buffer with its own layers,
/// over the sprite under it, where its particles cover the texel.
#[test]
fn particles_are_drawn_into_the_light_layers_on_their_emitter_s() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let size = 32;
    let target = RenderTarget::new(
        &device,
        "layers-target",
        voltra_render::light_layers::LAYER_FORMAT,
        size,
        size,
        Filter::Nearest,
    );
    let (pipelines, camera, layout) = pipelines(&device);
    camera.upload(&queue, &Camera2D::new(Vec2::ZERO, 1.0, 1.0));
    let white = Texture::white(&device, &queue).create_bind_group(&device, &layout);
    let opaque = [1.0; 4];
    let vertices = [
        Vertex::new([-1.0, 1.0], opaque, [0.0, 0.0]),
        Vertex::new([-1.0, -1.0], opaque, [0.0, 1.0]),
        Vertex::new([1.0, -1.0], opaque, [1.0, 1.0]),
        Vertex::new([1.0, 1.0], opaque, [1.0, 0.0]),
    ];
    let mesh = Mesh::indexed(&device, "backdrop", &vertices, &[0, 1, 2, 0, 2, 3]);

    // One particle a unit across, over the middle.
    let particles = GpuParticles::new(&device, &pipelines, 4);
    step(
        &device,
        &queue,
        &pipelines,
        &particles,
        &SimStep {
            spawn_count: 1,
            origin: Vec2::ZERO,
            lifetime: (10.0, 10.0),
            ..SimStep::default()
        },
    );
    particles.set_look(
        &queue,
        &ParticleLook {
            size: (1.0, 1.0),
            ..ParticleLook::default()
        },
    );
    let draws = [
        SceneDraw::Mesh(MeshDraw {
            texture: &white,
            normal: None,
            light_layers: 0b1,
            indices: 0..6,
        }),
        SceneDraw::Particles {
            particles: &particles,
            texture: &white,
            light_layers: 0b100,
        },
    ];
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("particle-layers"),
    });
    LayerPass::new(&device, camera.layout(), &layout, Some(&pipelines)).record(
        &device,
        &mut encoder,
        target.view(),
        camera.bind_group(),
        Some(&mesh),
        &draws,
    );
    queue.submit(Some(encoder.finish()));
    // Four bytes a texel either way: the excluded layers, little-endian.
    let pixels = read_texture(&device, &queue, target.texture(), size, size);
    let excluded = |x: u32, y: u32| {
        let p = pixels[(y * size + x) as usize];
        u32::from_le_bytes([p.r, p.g, p.b, p.a])
    };
    assert_eq!(excluded(16, 16), !0b100, "the particle's layers");
    assert_eq!(excluded(2, 2), !0b1, "the sprite's, where no particle is");
}
