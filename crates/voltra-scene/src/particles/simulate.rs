//! Ageing, moving and emitting, once per frame.

use voltra_ecs::{Entity, World};
use voltra_render::glam::{Mat3, Vec2};

use voltra_render::ParticleCollider;

use super::collide::{collide, Space};
use super::rng::Rng;
use super::{shape, EmitShape, Obstacles, Particle, ParticleEmitter, Simulation, MAX_PARTICLES};
use crate::hierarchy::WorldTransforms2D;
use crate::Collider2D;

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
    advance_where(world, dt, gpu, |_| true)
}

/// [`advance`] for only the emitters `which` picks — an editor previewing
/// the selected emitter while the rest of the scene holds still. The
/// finished one-shots it returns are among those.
pub fn advance_where(
    world: &mut World,
    dt: f32,
    gpu: bool,
    which: impl Fn(Entity) -> bool,
) -> Vec<Entity> {
    if !dt.is_finite() || dt <= 0.0 {
        return Vec::new();
    }
    let dt = dt.min(MAX_STEP);
    let transforms = WorldTransforms2D::from_world(world);
    // Gathered only when some emitter collides: a scene of plain fountains
    // pays nothing for the tiles it never looks at.
    let obstacles = if world
        .query::<ParticleEmitter>()
        .any(|(entity, emitter)| emitter.collision.is_some() && which(entity))
    {
        Obstacles::gather(world)
    } else {
        Obstacles::default()
    };
    // The outlines born on, read before the emitters are borrowed: an
    // emitter's collider is its own entity's, wherever it now is.
    let outlines: Vec<(Entity, Vec<[f32; 4]>)> = world
        .query::<ParticleEmitter>()
        .filter(|(entity, emitter)| emitter.shape == EmitShape::Outline && which(*entity))
        .map(|(entity, _)| {
            (
                entity,
                shape::collider_outline(world.get::<Collider2D>(entity)),
            )
        })
        .collect();
    for (entity, outline) in outlines {
        if let Some(emitter) = world.get_mut::<ParticleEmitter>(entity) {
            emitter.state.outline = outline;
        }
    }
    let mut scratch = Scratch::default();
    let mut finished = Vec::new();
    for (entity, emitter) in world.query_mut::<ParticleEmitter>() {
        if !which(entity) {
            continue;
        }
        if emitter.shape != EmitShape::Outline && !emitter.state.outline.is_empty() {
            emitter.state.outline.clear();
        }
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
    scratch.before.clear();
    // Velocity, then the distance it covers scaled by the speed curve at the
    // particle's new age: `particles_sim.wgsl`'s order.
    let speed_curve = emitter.speed_curve.as_ref();
    for particle in &mut emitter.state.particles {
        scratch.before.push(particle.position);
        particle.age += dt;
        let t = particle.progress();
        let speed = finite_or(speed_curve.map_or(1.0, |curve| curve.sample(t)), 0.0);
        particle.velocity += acceleration * dt;
        particle.position += particle.velocity * (speed * dt);
        particle.rotation += particle.spin * dt;
    }
    let state = &mut emitter.state;
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
        let (a, b) = (rng.unit(), rng.unit());
        let offset = emitter.shape.offset(&state.outline, a, b);
        let rotation = rng.between(emitter.rotation.0, emitter.rotation.1);
        let spin = rng.between(emitter.angular_velocity.0, emitter.angular_velocity.1);
        let (position, velocity) = if emitter.world_space {
            (
                matrix.transform_point2(offset),
                matrix.transform_vector2(local_velocity),
            )
        } else {
            (offset, local_velocity)
        };
        if !(position.is_finite() && velocity.is_finite() && lifetime.is_finite()) {
            continue;
        }
        state.particles.push(Particle {
            position,
            velocity,
            age: 0.0,
            lifetime,
            rotation: finite_or(rotation, 0.0),
            spin: finite_or(spin, 0.0),
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

fn finite_or(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}
