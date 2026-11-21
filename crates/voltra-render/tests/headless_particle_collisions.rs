//! GPU particles against colliders, read back from the ring: a hit puts a
//! particle just outside the surface and turns it as the response says,
//! one born inside dies, and a local-space ring collides where it is in the
//! world. The numbers are the CPU twin's (`voltra-scene`'s `collide`
//! tests), so the two simulations agree.
//!
//! Skips itself when no GPU adapter is available.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use voltra_render::glam::{Mat3, Vec2};
use voltra_render::particles::PARTICLE_SIZE;
use voltra_render::{
    texture, wgpu, CameraBinding, GpuParticles, ParticleCollider, ParticlePipelines,
    ParticleResponse, SimStep,
};
use voltra_testkit::{headless_device, read_buffer};

/// `SKIN` in `particles_sim.wgsl`.
const SKIN: f32 = 1e-3;

/// A floor whose top is at y = -0.5.
const FLOOR: ParticleCollider = ParticleCollider::Box {
    center: Vec2::new(0.0, -1.0),
    rotation: 0.0,
    half_extents: Vec2::new(5.0, 0.5),
};

#[derive(Debug, Clone, Copy)]
struct Slot {
    position: Vec2,
    velocity: Vec2,
    lifetime: f32,
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipelines: ParticlePipelines,
}

impl Gpu {
    fn new() -> Option<Self> {
        let (device, queue) = headless_device()?;
        let camera = CameraBinding::new(&device);
        let layout = texture::bind_group_layout(&device);
        let pipelines = ParticlePipelines::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            camera.layout(),
            &layout,
        );
        Some(Self {
            device,
            queue,
            pipelines,
        })
    }

    fn ring(&self, colliders: &[ParticleCollider]) -> GpuParticles {
        let mut ring = GpuParticles::new(&self.device, &self.pipelines, 4);
        ring.set_colliders(&self.device, &self.queue, &self.pipelines, colliders);
        ring
    }

    fn step(&self, ring: &GpuParticles, step: &SimStep) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("collision-test"),
            });
        ring.simulate(&self.queue, &mut encoder, &self.pipelines, step);
        self.queue.submit(Some(encoder.finish()));
    }

    fn first(&self, ring: &GpuParticles) -> Slot {
        let bytes = read_buffer(&self.device, &self.queue, ring.buffer(), PARTICLE_SIZE);
        let f: &[f32] = bytemuck::cast_slice(&bytes);
        Slot {
            position: Vec2::new(f[0], f[1]),
            velocity: Vec2::new(f[2], f[3]),
            lifetime: f[5],
        }
    }
}

/// One particle born at `origin` heading `direction` at `speed`, then one
/// step of `dt` with `response`.
#[allow(clippy::too_many_arguments)]
fn fire(
    gpu: &Gpu,
    ring: &GpuParticles,
    origin: Vec2,
    direction: f32,
    speed: f32,
    dt: f32,
    response: ParticleResponse,
    to_world: Mat3,
) -> Slot {
    gpu.step(
        ring,
        &SimStep {
            spawn_count: 1,
            origin,
            direction,
            speed: (speed, speed),
            lifetime: (1.0, 1.0),
            to_world,
            response,
            ..SimStep::default()
        },
    );
    gpu.step(
        ring,
        &SimStep {
            dt,
            to_world,
            response,
            ..SimStep::default()
        },
    );
    gpu.first(ring)
}

fn bounce(bounce: f32, friction: f32) -> ParticleResponse {
    ParticleResponse {
        bounce,
        friction,
        lifetime_loss: 0.0,
    }
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a - b).length() < 1e-4
}

#[test]
fn a_particle_falling_onto_a_box_bounces_off_its_top() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let ring = gpu.ring(&[FLOOR]);
    let p = fire(
        &gpu,
        &ring,
        Vec2::ZERO,
        -FRAC_PI_2,
        10.0,
        0.1,
        bounce(0.5, 0.0),
        Mat3::IDENTITY,
    );
    assert!(close(p.position, Vec2::new(0.0, -0.5 + SKIN)), "{p:?}");
    assert!(close(p.velocity, Vec2::new(0.0, 5.0)), "{p:?}");
    assert_eq!(p.lifetime, 1.0);
}

#[test]
fn a_circle_and_a_turned_box_turn_a_particle_along_their_normals() {
    let Some(gpu) = Gpu::new() else {
        return;
    };
    let ball = ParticleCollider::Circle {
        center: Vec2::new(2.0, 0.0),
        radius: 1.0,
    };
    let p = fire(
        &gpu,
        &gpu.ring(&[ball]),
        Vec2::ZERO,
        0.0,
        10.0,
        0.2,
        bounce(1.0, 0.0),
        Mat3::IDENTITY,
    );
    assert!(close(p.position, Vec2::new(1.0 - SKIN, 0.0)), "{p:?}");
    assert!(close(p.velocity, Vec2::new(-10.0, 0.0)), "{p:?}");

    let diamond = ParticleCollider::Box {
        center: Vec2::ZERO,
        rotation: FRAC_PI_4,
        half_extents: Vec2::ONE,
    };
    let p = fire(
        &gpu,
        &gpu.ring(&[diamond]),
        Vec2::splat(2.0),
        -3.0 * FRAC_PI_4,
        2.0_f32.sqrt(),
        2.0,
        bounce(1.0, 0.0),
        Mat3::IDENTITY,
    );
    let face = Vec2::splat(std::f32::consts::FRAC_1_SQRT_2);
    assert!(close(p.position, face * (1.0 + SKIN)), "{p:?}");
    assert!(close(p.velocity, Vec2::splat(1.0)), "{p:?}");
}

#[test]
fn a_particle_starting_inside_a_shape_dies() {
    let Some(gpu) = Gpu::new() else {
        return;
    };
    let p = fire(
        &gpu,
        &gpu.ring(&[FLOOR]),
        Vec2::new(0.0, -1.0),
        0.0,
        1.0,
        0.1,
        bounce(1.0, 0.0),
        Mat3::IDENTITY,
    );
    assert_eq!(p.lifetime, 0.0);
}

#[test]
fn a_local_space_ring_collides_where_it_is_in_the_world() {
    let Some(gpu) = Gpu::new() else {
        return;
    };
    // As the CPU test: the emitter 10 up and scaled 2×, the particle at
    // local (0, -5) — world (0, 0) — falling at 5 local units a second.
    let to_world = Mat3::from_scale_angle_translation(Vec2::splat(2.0), 0.0, Vec2::new(0.0, 10.0));
    let p = fire(
        &gpu,
        &gpu.ring(&[FLOOR]),
        Vec2::new(0.0, -5.0),
        -FRAC_PI_2,
        5.0,
        0.1,
        bounce(1.0, 0.0),
        to_world,
    );
    assert!(
        close(p.position, Vec2::new(0.0, -5.25 + SKIN / 2.0)),
        "{p:?}"
    );
    assert!(close(p.velocity, Vec2::new(0.0, 5.0)), "{p:?}");
}

#[test]
fn colliders_grow_the_buffer_and_invalid_ones_are_left_out() {
    let Some(gpu) = Gpu::new() else {
        return;
    };
    let mut ring = gpu.ring(&[]);
    assert_eq!(ring.collider_count(), 0);
    let mut many = vec![FLOOR; 5];
    many.push(ParticleCollider::Circle {
        center: Vec2::ZERO,
        radius: 0.0,
    });
    ring.set_colliders(&gpu.device, &gpu.queue, &gpu.pipelines, &many);
    assert_eq!(ring.collider_count(), 5);
    ring.set_colliders(&gpu.device, &gpu.queue, &gpu.pipelines, &[]);
    assert_eq!(ring.collider_count(), 0);

    // Nothing to hit: the particle flies through where the floor was.
    let p = fire(
        &gpu,
        &ring,
        Vec2::ZERO,
        -FRAC_PI_2,
        10.0,
        0.1,
        bounce(1.0, 0.0),
        Mat3::IDENTITY,
    );
    assert!(close(p.position, Vec2::new(0.0, -1.0)), "{p:?}");
}
