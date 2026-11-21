//! Ageing, moving and emitting, once per frame.

use voltra_ecs::{Entity, World};
use voltra_render::glam::{Mat3, Vec2};

use voltra_render::ParticleCollider;

use super::collide::{collide, Space};
use super::rng::Rng;
use super::{Obstacles, Particle, ParticleEmitter, Simulation, MAX_PARTICLES};
use crate::hierarchy::WorldTransforms;

/// The longest step simulated at once. A frame that stalls for a second must
/// not fire a second's worth of particles in one place.
const MAX_STEP: f32 = 0.1;

/// Advances every emitter by `dt` seconds. Returns the one-shot emitters
/// that have finished, for the caller to despawn.
///
/// `gpu` says whether the renderer can step particles: without it an
/// emitter set to [`Simulation::Gpu`](super::Simulation::Gpu) runs on the
/// CPU instead. A GPU emitter's step is planned here and dispatched by the
/// renderer — see [`ParticleEmitter::take_gpu_step`].
pub fn advance(world: &mut World, dt: f32, gpu: bool) -> Vec<Entity> {
    if !dt.is_finite() || dt <= 0.0 {
        return Vec::new();
    }
    let dt = dt.min(MAX_STEP);
    let transforms = WorldTransforms::from_world(world);
    // Gathered only when some emitter collides: a scene of plain fountains
    // pays nothing for the tiles it never looks at.
    let obstacles = if world
        .query::<ParticleEmitter>()
        .any(|(_, emitter)| emitter.collision.is_some())
    {
        Obstacles::gather(world)
    } else {
        Obstacles::default()
    };
    let mut scratch = Scratch::default();
    let mut finished = Vec::new();
    for (entity, emitter) in world.query_mut::<ParticleEmitter>() {
        let matrix = transforms.matrix(entity);
        step(emitter, entity, matrix, dt, gpu, &obstacles, &mut scratch);
        if emitter.is_finished() {
            finished.push(entity);
        }
    }
    finished
}

/// Buffers kept across emitters within one advance, so a colliding emitter
/// does not allocate per step.
#[derive(Default)]
struct Scratch {
    before: Vec<Vec2>,
    shapes: Vec<ParticleCollider>,
}

fn step(
    emitter: &mut ParticleEmitter,
    entity: Entity,
    matrix: Mat3,
    dt: f32,
    gpu: bool,
    obstacles: &Obstacles,
    scratch: &mut Scratch,
) {
    let acceleration = finite_or_zero(emitter.acceleration);
    let state = &mut emitter.state;
    scratch.before.clear();
    for particle in &mut state.particles {
        scratch.before.push(particle.position);
        particle.age += dt;
        particle.velocity += acceleration * dt;
        particle.position += particle.velocity * dt;
    }
    let space = Space::new(if emitter.world_space {
        Mat3::IDENTITY
    } else {
        matrix
    });
    if let (Some(collision), Some(space), false) = (emitter.collision, space, obstacles.is_empty())
    {
        // Only the shapes this step's paths can touch.
        let (min, max) = state
            .particles
            .iter()
            .zip(&scratch.before)
            .flat_map(|(p, &before)| [before, p.position])
            .map(|at| space.to_world(at))
            .fold((Vec2::INFINITY, Vec2::NEG_INFINITY), |(lo, hi), at| {
                (lo.min(at), hi.max(at))
            });
        obstacles.near(collision.mask, min, max, &mut scratch.shapes);
        let response = collision.response();
        for (particle, &before) in state.particles.iter_mut().zip(&scratch.before) {
            collide(particle, before, &scratch.shapes, &response, &space);
        }
    }
    state.particles.retain(|p| p.age < p.lifetime);

    let rate = if emitter.rate.is_finite() {
        emitter.rate.max(0.0)
    } else {
        0.0
    };
    let mut births = std::mem::take(&mut state.burst);
    if emitter.emitting {
        state.owed += rate * dt;
        let whole = state.owed.floor();
        state.owed -= whole;
        births = births.saturating_add(whole as u32);
    }
    if gpu && emitter.simulation == Simulation::Gpu {
        // Whatever the CPU still holds from before the switch dies out
        // above; every birth from here on is the GPU's.
        let mut plan = std::mem::take(&mut emitter.state.gpu);
        plan.step(emitter, matrix, dt, births, obstacles);
        emitter.state.gpu = plan;
        return;
    }
    if emitter.state.gpu.capacity > 0 {
        // Back on the CPU: the ring is the renderer's to drop.
        emitter.state.gpu = Default::default();
    }
    let state = &mut emitter.state;
    let cap = emitter.max_particles.min(MAX_PARTICLES) as usize;
    let room = cap.saturating_sub(state.particles.len());
    let births = (births as usize).min(room);

    let rng = state
        .rng
        .get_or_insert_with(|| Rng::new(u64::from(entity.index()) << 32 | 0x5eed));
    for _ in 0..births {
        let lifetime = rng.between(emitter.lifetime.0, emitter.lifetime.1).max(0.0);
        let speed = rng.between(emitter.speed.0, emitter.speed.1);
        let angle = emitter.direction + (rng.unit() - 0.5) * emitter.spread;
        let local_velocity = Vec2::from_angle(angle) * speed;
        let (position, velocity) = if emitter.world_space {
            (
                matrix.transform_point2(Vec2::ZERO),
                matrix.transform_vector2(local_velocity),
            )
        } else {
            (Vec2::ZERO, local_velocity)
        };
        if !(position.is_finite() && velocity.is_finite() && lifetime.is_finite()) {
            continue;
        }
        state.particles.push(Particle {
            position,
            velocity,
            age: 0.0,
            lifetime,
        });
    }
}

fn finite_or_zero(v: Vec2) -> Vec2 {
    if v.is_finite() {
        v
    } else {
        Vec2::ZERO
    }
}
