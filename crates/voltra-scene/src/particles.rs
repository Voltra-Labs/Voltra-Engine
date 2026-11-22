//! Particle emitters: many short-lived quads from one component.
//!
//! CPU-side and 2D, drawn by the sprite pipeline like everything else — a
//! particle is a tinted, textured quad that sorts with the sprites by its
//! emitter's `sort_order`. Unity's `ParticleSystem` and Godot's
//! `CPUParticles2D` cover the same ground; this is the subset a 2D game
//! reaches for first: a continuous rate, bursts, spread, gravity-like
//! acceleration, and size and colour from birth to death.

mod collide;
pub mod collision;
pub mod gpu;
pub mod mesh;
pub mod obstacles;
pub mod rng;
pub mod simulate;

use voltra_assets::{AssetPath, Handle, Textures};
use voltra_render::glam::Vec2;
use voltra_render::Texture;

pub use collision::ParticleCollision;
pub use obstacles::Obstacles;
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
    /// The named layer it draws on, before `sort_order` is consulted. See
    /// [`SortingLayers`](crate::sorting::SortingLayers).
    #[serde(
        default = "crate::sorting::default_layer",
        skip_serializing_if = "crate::sorting::is_default_layer"
    )]
    pub sorting_layer: String,
    /// The light layers its particles are on, as a sprite's
    /// [`light_layers`](crate::Sprite::light_layers) are. The first — the
    /// default, and not written — is what every light lights unless told
    /// otherwise.
    #[serde(
        default = "crate::light::default_light_layers",
        skip_serializing_if = "crate::light::is_default_light_layers"
    )]
    pub light_layers: u32,
    /// Particles keep the world position they were born at — sparks left
    /// behind a moving ship — rather than travelling with the emitter.
    pub world_space: bool,
    /// Despawn the entity once it is not emitting, owes no burst and has no
    /// particle left: a one-off explosion that cleans up after itself.
    #[serde(default)]
    pub one_shot: bool,
    /// Where the particles are stepped. The same parameters either way.
    #[serde(default, skip_serializing_if = "Simulation::is_cpu")]
    pub simulation: Simulation,
    /// Whether particles hit the world's colliders, and how they answer.
    /// `None` passes through everything. The same either simulation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collision: Option<ParticleCollision>,
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
            sorting_layer: crate::sorting::default_layer(),
            light_layers: crate::light::default_light_layers(),
            world_space: true,
            one_shot: false,
            simulation: Simulation::Cpu,
            collision: None,
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

    /// How many particles are alive — for a GPU emitter, the most that can
    /// be: the CPU counts births and lifetimes, not the buffer.
    pub fn alive(&self) -> usize {
        self.state.particles.len() + self.state.gpu.alive_bound()
    }

    /// Every particle alive, for a caller that draws or tests them.
    pub fn particles(&self) -> &[Particle] {
        &self.state.particles
    }

    /// Whether a one-shot emitter has nothing left to do.
    pub fn is_finished(&self) -> bool {
        self.one_shot
            && !self.emitting
            && self.state.burst == 0
            && self.state.particles.is_empty()
            && self.state.gpu.is_empty()
    }

    /// The ring a GPU emitter's renderer buffer must hold, or `None` while
    /// it runs on the CPU.
    pub fn gpu_capacity(&self) -> Option<u32> {
        (self.state.gpu.capacity > 0).then_some(self.state.gpu.capacity)
    }

    /// A box every live GPU particle is inside, in the world, with the
    /// emitter's world matrix at `matrix`; `None` when none can be alive.
    /// Conservative, for culling — see [`gpu::GpuPlan`].
    pub fn gpu_bounds(
        &self,
        matrix: voltra_render::glam::Mat3,
    ) -> Option<(voltra_render::glam::Vec2, voltra_render::glam::Vec2)> {
        self.state.gpu.bounds(self.world_space, matrix)
    }

    /// The dispatch owed since the last call: every step since, merged.
    /// `None` when nothing was stepped, or on the CPU.
    pub fn take_gpu_step(&mut self) -> Option<voltra_render::SimStep> {
        self.state.gpu.take()
    }

    /// The world's shapes the GPU particles may reach, as of the last step:
    /// what the renderer's ring tests them against. Empty without
    /// [`collision`](Self::collision), or on the CPU.
    pub fn gpu_colliders(&self) -> &[voltra_render::ParticleCollider] {
        self.state.gpu.colliders()
    }

    /// How the GPU particles are drawn, placed by the emitter's world
    /// `matrix`.
    pub fn gpu_look(&self, matrix: voltra_render::glam::Mat3) -> voltra_render::ParticleLook {
        gpu::look(self, matrix)
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

/// Where an emitter's particles are stepped and drawn from.
///
/// The CPU is the default and the reference: exact counts, no device
/// needed, every particle readable. The GPU takes the per-particle work —
/// tens of thousands cost what a handful do — with the same parameters and
/// look; the CPU still decides how many are born and when. An adapter
/// without compute shaders runs a GPU emitter on the CPU instead.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Simulation {
    #[default]
    Cpu,
    Gpu,
}

impl Simulation {
    pub fn is_cpu(&self) -> bool {
        *self == Self::Cpu
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
    pub(crate) gpu: gpu::GpuPlan,
}

#[cfg(test)]
mod collision_tests;
#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod tests;
