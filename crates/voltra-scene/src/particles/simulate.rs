//! Ageing, moving and emitting, once per frame.

use voltra_ecs::{Entity, World};
use voltra_render::glam::{Mat3, Vec2};

use super::rng::Rng;
use super::{Particle, ParticleEmitter, MAX_PARTICLES};
use crate::hierarchy::WorldTransforms;

/// The longest step simulated at once. A frame that stalls for a second must
/// not fire a second's worth of particles in one place.
const MAX_STEP: f32 = 0.1;

/// Advances every emitter by `dt` seconds. Returns the one-shot emitters
/// that have finished, for the caller to despawn.
pub fn advance(world: &mut World, dt: f32) -> Vec<Entity> {
    if !dt.is_finite() || dt <= 0.0 {
        return Vec::new();
    }
    let dt = dt.min(MAX_STEP);
    let transforms = WorldTransforms::from_world(world);
    let mut finished = Vec::new();
    for (entity, emitter) in world.query_mut::<ParticleEmitter>() {
        step(emitter, entity, transforms.matrix(entity), dt);
        if emitter.is_finished() {
            finished.push(entity);
        }
    }
    finished
}

fn step(emitter: &mut ParticleEmitter, entity: Entity, matrix: Mat3, dt: f32) {
    let acceleration = finite_or_zero(emitter.acceleration);
    let state = &mut emitter.state;
    for particle in &mut state.particles {
        particle.age += dt;
        particle.velocity += acceleration * dt;
        particle.position += particle.velocity * dt;
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
