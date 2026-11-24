//! What a step of the simulation and a draw of the particles are given.

use glam::{Mat3, Vec2};

use super::colliders::ParticleResponse;
use super::spawn_shape::SpawnShape;

/// One step of one emitter: how long it lasts, which slots are born in it,
/// and what a newborn is drawn from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimStep {
    /// Seconds every live particle is moved on.
    pub dt: f32,
    /// The first slot born this step; the next `spawn_count` follow it,
    /// wrapping round the ring.
    pub spawn_start: u32,
    /// How many slots are born this step.
    pub spawn_count: u32,
    /// Varies newborns from one step to the next.
    pub seed: u32,
    /// Where a newborn starts: the emitter's world position for a
    /// world-space emitter, the origin of its own space otherwise.
    pub origin: Vec2,
    /// The emitter's x and y axes, which a world-space newborn's velocity is
    /// turned and scaled by; the identity's axes for a local-space one.
    pub basis: [Vec2; 2],
    /// Radians from +x, and the width of the cone around it.
    pub direction: f32,
    /// Width of the cone, in radians.
    pub spread: f32,
    /// Range a newborn's speed is drawn from, as (min, max).
    pub speed: (f32, f32),
    /// Range a newborn's lifetime is drawn from, in seconds, as (min, max).
    pub lifetime: (f32, f32),
    /// Added to every live particle's velocity, per second.
    pub acceleration: Vec2,
    /// Where round `origin` a newborn starts, in the emitter's space
    /// (through `basis` for a world-space emitter).
    pub shape: SpawnShape,
    /// Range a newborn's angle is drawn from, in radians.
    pub rotation: (f32, f32),
    /// Range a newborn's spin is drawn from, in radians per second.
    pub angular_velocity: (f32, f32),
    /// Takes a particle's position and velocity into the world, where the
    /// colliders are: the identity for a world-space emitter, the emitter's
    /// world matrix for a local-space one.
    pub to_world: Mat3,
    /// What a particle does on hitting one of the ring's colliders.
    pub response: ParticleResponse,
}

impl Default for SimStep {
    /// A step that moves nothing and births nothing.
    fn default() -> Self {
        Self {
            dt: 0.0,
            spawn_start: 0,
            spawn_count: 0,
            seed: 0,
            origin: Vec2::ZERO,
            basis: [Vec2::X, Vec2::Y],
            direction: 0.0,
            spread: 0.0,
            speed: (0.0, 0.0),
            lifetime: (0.0, 0.0),
            acceleration: Vec2::ZERO,
            shape: SpawnShape::Point,
            rotation: (0.0, 0.0),
            angular_velocity: (0.0, 0.0),
            to_world: Mat3::IDENTITY,
            response: ParticleResponse::default(),
        }
    }
}

/// How a flipbook's frames follow a particle's life.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FrameTiming {
    /// Through every frame `cycles` times from birth to death.
    OverLife {
        /// Passes through the frames in one life.
        cycles: f32,
    },
    /// At a rate, from the first frame at birth, round again after the last.
    PerSecond {
        /// Frames a second.
        fps: f32,
    },
    /// No flipbook: the whole texture.
    #[default]
    None,
}

/// How an emitter's particles are placed and animated. What they look like
/// over their life is in its [`ParticleTables`](super::ParticleTables).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleLook {
    /// Applied to every particle position: the emitter's world matrix for a
    /// local-space emitter, the identity for a world-space one.
    pub place: Mat3,
    /// How the tables' flipbook frames are stepped through.
    pub frames: FrameTiming,
}

impl Default for ParticleLook {
    fn default() -> Self {
        Self {
            place: Mat3::IDENTITY,
            frames: FrameTiming::None,
        }
    }
}

/// [`SimStep`] as `particles_sim.wgsl` reads it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct SimUniform {
    origin_dt: [f32; 4],
    basis: [f32; 4],
    ranges: [f32; 4],
    accel_dir: [f32; 4],
    spawn: [u32; 4],
    /// to_world's x axis .xy and y axis .zw, then its translation .xy; the
    /// inverse the same way.
    to_world: [f32; 4],
    to_world_t: [f32; 4],
    from_world: [f32; 4],
    from_world_t: [f32; 4],
    /// bounce, friction, lifetime loss, unused
    response: [f32; 4],
    /// colliders, shape kind (0 point, 1 ring, 2 box, 3 edge, 4 outline),
    /// outline corners, unused
    counts: [u32; 4],
    /// The shape's sizes: ring inner, outer; box half x, y; edge half length.
    shape: [f32; 4],
    /// rotation min, max, angular velocity min, max
    spin: [f32; 4],
}

impl SimUniform {
    /// Nonsense in, nothing out: a non-finite value is zero (a NaN speed
    /// would put a particle nowhere, forever), a negative step does not run
    /// the clock backwards, and no more are born than the ring holds. A
    /// `to_world` that cannot be undone — scaled to nothing — tests none of
    /// the `colliders`: there is no way back from the world to the particle.
    /// An outline of fewer than two corners births at the origin.
    pub(super) fn new(step: &SimStep, capacity: u32, colliders: u32, outline: u32) -> Self {
        let f = |v: f32| if v.is_finite() { v } else { 0.0 };
        let v = |v: Vec2| if v.is_finite() { v } else { Vec2::ZERO };
        let origin = v(step.origin);
        let [x, y] = step.basis.map(v);
        let acceleration = v(step.acceleration);
        let invertible = step.to_world.is_finite() && step.to_world.determinant().abs() > 1e-12;
        let (to_world, from_world, colliders) = if invertible {
            (step.to_world, step.to_world.inverse(), colliders)
        } else {
            (Mat3::IDENTITY, Mat3::IDENTITY, 0)
        };
        let axes = |m: Mat3| [m.x_axis.x, m.x_axis.y, m.y_axis.x, m.y_axis.y];
        let translation = |m: Mat3| [m.z_axis.x, m.z_axis.y, 0.0, 0.0];
        let response = step.response.clamped();
        let (kind, shape) = match step.shape {
            SpawnShape::Point => (0, [0.0; 4]),
            SpawnShape::Ring { inner, outer } => {
                let (inner, outer) = (f(inner).abs(), f(outer).abs());
                (1, [inner.min(outer), inner.max(outer), 0.0, 0.0])
            }
            SpawnShape::Box { half_extents } => {
                let half = v(half_extents).abs();
                (2, [half.x, half.y, 0.0, 0.0])
            }
            SpawnShape::Edge { half_length } => (3, [f(half_length).abs(), 0.0, 0.0, 0.0]),
            SpawnShape::Outline if outline >= 2 => (4, [0.0; 4]),
            SpawnShape::Outline => (0, [0.0; 4]),
        };
        Self {
            origin_dt: [origin.x, origin.y, f(step.dt).max(0.0), f(step.spread)],
            basis: [x.x, x.y, y.x, y.y],
            ranges: [
                f(step.speed.0),
                f(step.speed.1),
                f(step.lifetime.0).max(0.0),
                f(step.lifetime.1).max(0.0),
            ],
            accel_dir: [acceleration.x, acceleration.y, f(step.direction), 0.0],
            spawn: [
                step.spawn_start % capacity.max(1),
                step.spawn_count.min(capacity),
                capacity,
                step.seed,
            ],
            to_world: axes(to_world),
            to_world_t: translation(to_world),
            from_world: axes(from_world),
            from_world_t: translation(from_world),
            response: [
                response.bounce,
                response.friction,
                response.lifetime_loss,
                0.0,
            ],
            counts: [colliders, kind, outline, 0],
            shape,
            spin: [
                f(step.rotation.0),
                f(step.rotation.1),
                f(step.angular_velocity.0),
                f(step.angular_velocity.1),
            ],
        }
    }
}

/// [`ParticleLook`] as `particles_draw.wgsl` reads it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct LookUniform {
    place_x: [f32; 4],
    place_y: [f32; 4],
    place_t: [f32; 4],
    /// frames, timing (0 whole texture, 1 over life, 2 per second), cycles
    /// or fps, unused
    frames: [f32; 4],
}

impl LookUniform {
    pub(super) fn new(look: &ParticleLook, frames: u32) -> Self {
        let f = |v: f32| if v.is_finite() { v } else { 0.0 };
        let place = if look.place.is_finite() {
            look.place
        } else {
            Mat3::IDENTITY
        };
        let column = |c: glam::Vec3| [c.x, c.y, 0.0, 0.0];
        let (mode, rate) = match look.frames {
            _ if frames == 0 => (0.0, 0.0),
            FrameTiming::None => (0.0, 0.0),
            FrameTiming::OverLife { cycles } => (1.0, f(cycles)),
            FrameTiming::PerSecond { fps } => (2.0, f(fps)),
        };
        Self {
            place_x: column(place.x_axis),
            place_y: column(place.y_axis),
            place_t: column(place.z_axis),
            frames: [frames as f32, mode, rate, 0.0],
        }
    }
}

#[cfg(test)]
mod tests;
