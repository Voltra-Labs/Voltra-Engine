//! What a step of the simulation and a draw of the particles are given.

use glam::{Mat3, Vec2};

use super::colliders::ParticleResponse;

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
            to_world: Mat3::IDENTITY,
            response: ParticleResponse::default(),
        }
    }
}

/// How an emitter's particles are drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleLook {
    /// Applied to every particle position: the emitter's world matrix for a
    /// local-space emitter, the identity for a world-space one.
    pub place: Mat3,
    /// Side of the square at birth and at death.
    pub size: (f32, f32),
    /// Linear RGBA at birth.
    pub color_start: [f32; 4],
    /// Linear RGBA at death.
    pub color_end: [f32; 4],
}

impl Default for ParticleLook {
    fn default() -> Self {
        Self {
            place: Mat3::IDENTITY,
            size: (0.0, 0.0),
            color_start: [1.0; 4],
            color_end: [1.0; 4],
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
    /// colliders, unused ×3
    colliders: [u32; 4],
}

impl SimUniform {
    /// Nonsense in, nothing out: a non-finite value is zero (a NaN speed
    /// would put a particle nowhere, forever), a negative step does not run
    /// the clock backwards, and no more are born than the ring holds. A
    /// `to_world` that cannot be undone — scaled to nothing — tests none of
    /// the `colliders`: there is no way back from the world to the particle.
    pub(super) fn new(step: &SimStep, capacity: u32, colliders: u32) -> Self {
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
            colliders: [colliders, 0, 0, 0],
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
    size: [f32; 4],
    color_start: [f32; 4],
    color_end: [f32; 4],
}

impl LookUniform {
    pub(super) fn new(look: &ParticleLook) -> Self {
        let f = |v: f32| if v.is_finite() { v } else { 0.0 };
        let place = if look.place.is_finite() {
            look.place
        } else {
            Mat3::IDENTITY
        };
        let column = |c: glam::Vec3| [c.x, c.y, 0.0, 0.0];
        Self {
            place_x: column(place.x_axis),
            place_y: column(place.y_axis),
            place_t: column(place.z_axis),
            size: [f(look.size.0), f(look.size.1), 0.0, 0.0],
            color_start: look.color_start.map(f),
            color_end: look.color_end.map(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonsense_becomes_nothing() {
        let step = SimStep {
            dt: -1.0,
            spawn_start: 10,
            spawn_count: 99,
            speed: (f32::NAN, 1.0),
            origin: Vec2::new(f32::INFINITY, 0.0),
            ..SimStep::default()
        };
        let u = SimUniform::new(&step, 8, 0);
        assert_eq!(u.origin_dt[..3], [0.0, 0.0, 0.0]);
        assert_eq!(u.ranges[0], 0.0);
        assert_eq!(u.spawn[..3], [2, 8, 8], "wrapped and capped to the ring");
    }

    #[test]
    fn an_empty_ring_does_not_divide_by_zero() {
        let u = SimUniform::new(&SimStep::default(), 0, 0);
        assert_eq!(u.spawn[..3], [0, 0, 0]);
    }

    #[test]
    fn a_space_scaled_to_nothing_tests_no_collider() {
        let flat = SimStep {
            to_world: Mat3::from_scale(Vec2::new(1.0, 0.0)),
            ..SimStep::default()
        };
        assert_eq!(SimUniform::new(&flat, 8, 3).colliders[0], 0);
        let moved = SimStep {
            to_world: Mat3::from_translation(Vec2::new(2.0, 0.0)),
            ..SimStep::default()
        };
        let u = SimUniform::new(&moved, 8, 3);
        assert_eq!(u.colliders[0], 3);
        assert_eq!(u.from_world_t[0], -2.0, "the inverse goes back");
    }

    #[test]
    fn a_broken_matrix_places_at_the_origin() {
        let look = ParticleLook {
            place: Mat3::from_cols_array(&[f32::NAN; 9]),
            ..ParticleLook::default()
        };
        assert_eq!(LookUniform::new(&look).place_x, [1.0, 0.0, 0.0, 0.0]);
    }
}
