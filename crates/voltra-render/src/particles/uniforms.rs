//! What a step of the simulation and a draw of the particles are given.

use glam::{Mat3, Vec2};

/// One step of one emitter: how long it lasts, which slots are born in it,
/// and what a newborn is drawn from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimStep {
    /// Seconds every live particle is moved on.
    pub dt: f32,
    /// The first slot born this step; the next `spawn_count` follow it,
    /// wrapping round the ring.
    pub spawn_start: u32,
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
    pub spread: f32,
    pub speed: (f32, f32),
    pub lifetime: (f32, f32),
    pub acceleration: Vec2,
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
    pub color_start: [f32; 4],
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
}

impl SimUniform {
    /// Nonsense in, nothing out: a non-finite value is zero (a NaN speed
    /// would put a particle nowhere, forever), a negative step does not run
    /// the clock backwards, and no more are born than the ring holds.
    pub(super) fn new(step: &SimStep, capacity: u32) -> Self {
        let f = |v: f32| if v.is_finite() { v } else { 0.0 };
        let v = |v: Vec2| if v.is_finite() { v } else { Vec2::ZERO };
        let origin = v(step.origin);
        let [x, y] = step.basis.map(v);
        let acceleration = v(step.acceleration);
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
        let u = SimUniform::new(&step, 8);
        assert_eq!(u.origin_dt[..3], [0.0, 0.0, 0.0]);
        assert_eq!(u.ranges[0], 0.0);
        assert_eq!(u.spawn[..3], [2, 8, 8], "wrapped and capped to the ring");
    }

    #[test]
    fn an_empty_ring_does_not_divide_by_zero() {
        let u = SimUniform::new(&SimStep::default(), 0);
        assert_eq!(u.spawn[..3], [0, 0, 0]);
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
