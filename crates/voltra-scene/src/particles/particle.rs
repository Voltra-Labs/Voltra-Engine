//! One particle, and what an emitter holds of them between frames.

use voltra_render::glam::Vec2;

use super::gpu;
use super::rng::Rng;

/// One particle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    /// In world space for a world-space emitter, in the emitter's space
    /// otherwise.
    pub position: Vec2,
    /// Units per second, in the space `position` is in.
    pub velocity: Vec2,
    /// Seconds since it was born.
    pub age: f32,
    /// Seconds it lives; it dies when `age` reaches it.
    pub lifetime: f32,
}

impl Particle {
    /// How far through its life, `0` at birth to `1` at death.
    pub fn progress(&self) -> f32 {
        if self.lifetime > 0.0 {
            (self.age / self.lifetime).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}

/// An emitter's live particles, the share of a particle the rate owes, the
/// bursts waiting and the generator picking every random value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Particles {
    pub(crate) particles: Vec<Particle>,
    pub(crate) owed: f32,
    pub(crate) burst: u32,
    pub(crate) rng: Option<Rng>,
    pub(crate) gpu: gpu::GpuPlan,
}
