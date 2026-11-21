//! What GPU particles collide with, and how they answer a hit.
//!
//! The shapes are world-space copies of the scene's colliders, picked per
//! emitter by the caller; the GPU tests every particle against every shape
//! it was given, so the caller hands over only those near the emitter.

use glam::Vec2;

/// A shape particles bounce off, in the world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParticleCollider {
    /// A box centred on `center`, turned `rotation` radians.
    Box {
        center: Vec2,
        rotation: f32,
        half_extents: Vec2,
    },
    Circle {
        center: Vec2,
        radius: f32,
    },
}

impl ParticleCollider {
    /// Whether every value is a number and the shape has area: what the
    /// shader can test without dividing by nothing.
    pub fn is_valid(&self) -> bool {
        match *self {
            Self::Box {
                center,
                rotation,
                half_extents,
            } => {
                center.is_finite()
                    && rotation.is_finite()
                    && half_extents.is_finite()
                    && half_extents.x > 0.0
                    && half_extents.y > 0.0
            }
            Self::Circle { center, radius } => {
                center.is_finite() && radius.is_finite() && radius > 0.0
            }
        }
    }
}

/// What a particle does when it hits a shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleResponse {
    /// The share of the speed into the surface that comes back out, `0` (a
    /// dead stop) to `1` (a perfect bounce).
    pub bounce: f32,
    /// The share of the speed along the surface lost on each hit, `0` to `1`.
    pub friction: f32,
    /// The share of its lifetime a particle loses on each hit, `0` to `1`:
    /// `1` dies on the first.
    pub lifetime_loss: f32,
}

impl ParticleResponse {
    /// Each share clamped into `0..=1`, a non-number as zero: a bounce past
    /// one would gain speed every hit, and the culling bound counts on
    /// speed never growing.
    pub fn clamped(&self) -> Self {
        let share = |v: f32| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        Self {
            bounce: share(self.bounce),
            friction: share(self.friction),
            lifetime_loss: share(self.lifetime_loss),
        }
    }
}

impl Default for ParticleResponse {
    /// No bounce, no friction, no lifetime lost: a particle that lands
    /// slides.
    fn default() -> Self {
        Self {
            bounce: 0.0,
            friction: 0.0,
            lifetime_loss: 0.0,
        }
    }
}

/// [`ParticleCollider`] as `particles_sim.wgsl` reads it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct ColliderGpu {
    /// centre.xy, cos, sin of the rotation
    place: [f32; 4],
    /// half extents.xy for a box, radius in x for a circle; kind (0 box, 1
    /// circle); unused
    shape: [f32; 4],
}

/// Bytes per shape in the storage buffer.
pub(super) const COLLIDER_SIZE: u64 = std::mem::size_of::<ColliderGpu>() as u64;

impl ColliderGpu {
    pub(super) fn new(collider: &ParticleCollider) -> Self {
        match *collider {
            ParticleCollider::Box {
                center,
                rotation,
                half_extents,
            } => {
                let (sin, cos) = rotation.sin_cos();
                Self {
                    place: [center.x, center.y, cos, sin],
                    shape: [half_extents.x, half_extents.y, 0.0, 0.0],
                }
            }
            ParticleCollider::Circle { center, radius } => Self {
                place: [center.x, center.y, 1.0, 0.0],
                shape: [radius, 0.0, 1.0, 0.0],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shape_without_area_or_numbers_is_not_valid() {
        assert!(ParticleCollider::Circle {
            center: Vec2::ZERO,
            radius: 1.0
        }
        .is_valid());
        assert!(!ParticleCollider::Circle {
            center: Vec2::ZERO,
            radius: 0.0
        }
        .is_valid());
        assert!(!ParticleCollider::Box {
            center: Vec2::NAN,
            rotation: 0.0,
            half_extents: Vec2::ONE
        }
        .is_valid());
    }

    #[test]
    fn shares_are_clamped_so_no_bounce_gains_speed() {
        let r = ParticleResponse {
            bounce: 2.0,
            friction: -1.0,
            lifetime_loss: f32::NAN,
        }
        .clamped();
        assert_eq!((r.bounce, r.friction, r.lifetime_loss), (1.0, 0.0, 0.0));
    }

    #[test]
    fn the_layout_is_two_vec4s() {
        assert_eq!(COLLIDER_SIZE, 32);
    }
}
