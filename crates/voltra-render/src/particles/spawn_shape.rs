//! Where round an emitter's origin the GPU births a particle.

use glam::Vec2;

/// Where in the emitter's space a newborn starts.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SpawnShape {
    /// At the origin.
    #[default]
    Point,
    /// Evenly over the area between two circles round the origin: a disc
    /// when `inner` is zero, a circle's edge when the two are equal.
    Ring {
        /// The radius inside which none is born.
        inner: f32,
        /// The radius outside which none is born.
        outer: f32,
    },
    /// Evenly over a rectangle round the origin.
    Box {
        /// Half its width and height.
        half_extents: Vec2,
    },
    /// Evenly along a line through the origin on the emitter's x axis.
    Edge {
        /// Half its length.
        half_length: f32,
    },
    /// Evenly along the outline in the emitter's
    /// [`ParticleTables::outline`](super::ParticleTables::outline); the
    /// origin when it has fewer than two corners.
    Outline,
}
