//! Particle emitters: many short-lived quads from one component.
//!
//! CPU-side and 2D, drawn by the sprite pipeline like everything else — a
//! particle is a tinted, textured quad that sorts with the sprites by its
//! emitter's `sort_order`. Unity's `ParticleSystem` and Godot's
//! `CPUParticles2D` cover the same ground; this is the subset a 2D game
//! reaches for first: a continuous rate, bursts, spread, gravity-like
//! acceleration, and size and colour from birth to death.

pub mod mesh;
pub mod rng;
pub mod simulate;

use voltra_assets::{AssetPath, Handle, Textures};
use voltra_render::glam::Vec2;
use voltra_render::Texture;

pub use simulate::advance;

use rng::Rng;

/// The most particles one emitter may hold, whatever its file says.
pub const MAX_PARTICLES: u32 = 10_000;

/// Emits particles from its entity's position.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ParticleEmitter {
    /// Whether the continuous `rate` is on. Bursts fire either way.
    pub emitting: bool,
    /// Particles per second while emitting.
    pub rate: f32,
    /// The most alive at once; further births wait for deaths.
    pub max_particles: u32,
    /// Seconds each particle lives, picked between the two.
    pub lifetime: (f32, f32),
    /// Starting speed in units per second, picked between the two.
    pub speed: (f32, f32),
    /// Radians, counter-clockwise from +x, in the emitter's space: `π/2` is
    /// up.
    pub direction: f32,
    /// The full width of the cone particles leave in, in radians: `0` is a
    /// line, `2π` every way.
    pub spread: f32,
    /// Units per second squared, every particle, every frame — gravity, or
    /// a rising drift for smoke.
    pub acceleration: Vec2,
    /// Side of a particle's square at birth and at death, in units.
    pub size: (f32, f32),
    /// Colour at birth and at death, alpha included: fading to an alpha of
    /// zero is how a particle disappears rather than blinking out.
    pub color_start: [f32; 4],
    pub color_end: [f32; 4],
    /// The image every particle draws, by path. `None` is a flat square.
    #[serde(default)]
    pub texture: Option<AssetPath>,
    #[serde(skip)]
    pub texture_handle: Option<Handle<Texture>>,
    #[serde(default)]
    pub sort_order: i32,
    /// Particles keep the world position they were born at — sparks left
    /// behind a moving ship — rather than travelling with the emitter.
    pub world_space: bool,
    /// Despawn the entity once it is not emitting, owes no burst and has no
    /// particle left: a one-off explosion that cleans up after itself.
    #[serde(default)]
    pub one_shot: bool,
    /// What is alive now. Never saved: a scene file says what an emitter
    /// is, not which sparks were in the air.
    ///
    /// Public only so `ParticleEmitter { rate: 5.0, ..Default::default() }`
    /// works outside this crate; its contents are not, and are read through
    /// [`particles`](Self::particles) and [`alive`](Self::alive).
    #[serde(skip)]
    pub state: Particles,
}

impl Default for ParticleEmitter {
    /// A small upward fountain of white squares that shrink and fade.
    fn default() -> Self {
        Self {
            emitting: true,
            rate: 20.0,
            max_particles: 256,
            lifetime: (0.8, 1.2),
            speed: (1.0, 2.0),
            direction: std::f32::consts::FRAC_PI_2,
            spread: 0.5,
            acceleration: Vec2::new(0.0, -2.0),
            size: (0.2, 0.0),
            color_start: [1.0, 1.0, 1.0, 1.0],
            color_end: [1.0, 1.0, 1.0, 0.0],
            texture: None,
            texture_handle: None,
            sort_order: 0,
            world_space: true,
            one_shot: false,
            state: Particles::default(),
        }
    }
}

impl ParticleEmitter {
    /// Emits `count` particles at once on the next advance, on top of the
    /// rate — an explosion, a coin's sparkle.
    pub fn burst(&mut self, count: u32) {
        self.state.burst = self.state.burst.saturating_add(count);
    }

    /// How many particles are alive.
    pub fn alive(&self) -> usize {
        self.state.particles.len()
    }

    /// Every particle alive, for a caller that draws or tests them.
    pub fn particles(&self) -> &[Particle] {
        &self.state.particles
    }

    /// Whether a one-shot emitter has nothing left to do.
    pub fn is_finished(&self) -> bool {
        self.one_shot && !self.emitting && self.state.burst == 0 && self.state.particles.is_empty()
    }

    /// Sets or clears the texture path and refreshes its handle, as
    /// [`Sprite::set_texture`](crate::Sprite::set_texture) does.
    pub fn set_texture(
        &mut self,
        path: Option<AssetPath>,
        textures: &mut Textures,
        device: &voltra_render::wgpu::Device,
        queue: &voltra_render::wgpu::Queue,
    ) {
        self.texture_handle = path.as_ref().map(|path| textures.load(device, queue, path));
        self.texture = path;
    }
}

/// One particle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    /// In world space for a world-space emitter, in the emitter's space
    /// otherwise.
    pub position: Vec2,
    pub velocity: Vec2,
    pub age: f32,
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
}

#[cfg(test)]
mod tests;
