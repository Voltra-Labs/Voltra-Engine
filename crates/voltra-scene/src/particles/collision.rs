//! An emitter's collision settings: what its particles hit and how they
//! answer.
//!
//! Unity's particle Collision module in its "World" mode, cut to the 2D
//! subset: particles hit the scene's [`Collider`](crate::Collider)s — boxes,
//! circles and the boxes generated from solid tiles — and bounce, slide or
//! die. Godot's `GPUParticles2D` collides against an SDF baked from light
//! occluders instead; this engine has colliders already, and a second set of
//! shapes to author for particles would drift from the first.

use voltra_render::ParticleResponse;

use crate::CollisionLayers;

/// How an emitter's particles collide with the world's colliders.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ParticleCollision {
    /// The share of the speed into a surface that comes back out: `0` lands
    /// dead, `1` bounces back as fast as it came.
    pub bounce: f32,
    /// The share of the speed along a surface lost on each hit.
    pub friction: f32,
    /// The share of its lifetime a particle loses on each hit: `1` dies on
    /// the first — rain that vanishes where it lands.
    pub lifetime_loss: f32,
    /// The collision layers hit, as a mask of
    /// [`CollisionLayers::layers`](crate::CollisionLayers): a collider is hit
    /// when it is on any of them. Particles are not on a layer themselves;
    /// nothing collides with them.
    pub mask: u32,
}

impl Default for ParticleCollision {
    /// Sparks that bounce at half their speed off everything.
    fn default() -> Self {
        Self {
            bounce: 0.5,
            friction: 0.1,
            lifetime_loss: 0.0,
            mask: CollisionLayers::ALL,
        }
    }
}

impl ParticleCollision {
    /// The response both simulations apply, each share clamped into `0..=1`.
    pub fn response(&self) -> ParticleResponse {
        ParticleResponse {
            bounce: self.bounce,
            friction: self.friction,
            lifetime_loss: self.lifetime_loss,
        }
        .clamped()
    }
}
