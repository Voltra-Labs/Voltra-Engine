//! Particles simulated and drawn on the GPU: one storage buffer per emitter,
//! aged and moved by a compute pass, drawn as instanced quads.
//!
//! Godot's `GPUParticles2D` and Unity's VFX Graph, cut to the CPU emitter's
//! parameters so an emitter can switch between the two without changing
//! what it looks like. The CPU decides **how many** are born each step and
//! where the ring's write cursor is — rate, bursts and "births wait for
//! room" stay one piece of code — and the GPU does the per-particle work:
//! picking each newborn's speed, angle and lifetime, and moving every one.
//!
//! The buffer is a ring of `capacity` slots. A step's newborns take the
//! slots from its `spawn_start`; the caller never hands out a slot whose
//! particle may still be alive, so nothing visible is overwritten.

mod buffer;
mod colliders;
mod pipelines;
mod uniforms;

pub use buffer::GpuParticles;
pub use colliders::{ParticleCollider, ParticleResponse};
pub use pipelines::ParticlePipelines;
pub use uniforms::{ParticleLook, SimStep};

/// The most particles one GPU emitter may hold.
pub const MAX_GPU_PARTICLES: u32 = 1 << 20;

/// Bytes per particle in the storage buffer: `particles_*.wgsl`'s struct.
pub const PARTICLE_SIZE: u64 = 32;
