//! Particle emitters: many short-lived quads from one component.
//!
//! 2D, drawn by the sprite pipeline like everything else — a particle is a
//! tinted, textured quad that sorts with the sprites by its emitter's
//! `sort_order` — or simulated and drawn on the GPU with the same look.
//! Unity's `ParticleSystem` and Godot's `CPUParticles2D` cover the same
//! ground; this is the subset a 2D game reaches for: a continuous rate,
//! bursts, spread, gravity-like acceleration, an emission shape, size,
//! speed and alpha along curves over a life, a colour gradient, spin and
//! flipbook frames.

mod collide;
pub mod collision;
pub mod flipbook;
pub mod gpu;
pub mod look;
pub mod mesh;
pub mod obstacles;
mod particle;
pub mod presets;
pub mod rng;
pub mod shape;
pub mod simulate;
mod simulation;

use voltra_assets::{AssetPath, Handle, Textures};
use voltra_render::glam::Vec2;
use voltra_render::Texture;

use crate::curve::Curve;
use crate::gradient::Gradient;

pub use collision::ParticleCollision;
pub use flipbook::{Flipbook, FlipbookTiming};
pub use obstacles::Obstacles;
pub use particle::{Particle, Particles};
pub use shape::EmitShape;
pub use simulate::{advance, advance_where};
pub use simulation::Simulation;

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
    /// Where round the emitter particles are born. A point by default.
    #[serde(default, skip_serializing_if = "EmitShape::is_point")]
    pub shape: EmitShape,
    /// Side of a particle's square at birth and at death, in units.
    pub size: (f32, f32),
    /// What the size is multiplied by over a life, `t` from `0` at birth to
    /// `1` at death. `None` changes nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_curve: Option<Curve>,
    /// What the distance a particle covers is multiplied by over its life:
    /// `1` is its velocity, `0` holds it still. `None` changes nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_curve: Option<Curve>,
    /// Colour at birth and at death, alpha included: fading to an alpha of
    /// zero is how a particle disappears rather than blinking out.
    pub color_start: [f32; 4],
    /// The colour at death, RGBA.
    pub color_end: [f32; 4],
    /// The colour over a life, in place of `color_start` to `color_end`
    /// when it has a stop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<Gradient>,
    /// What the alpha is multiplied by over a life. `None` changes nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha_curve: Option<Curve>,
    /// A newborn's angle, in radians, picked between the two.
    #[serde(default, skip_serializing_if = "is_zero_range")]
    pub rotation: (f32, f32),
    /// A newborn's spin, in radians per second, picked between the two.
    #[serde(default, skip_serializing_if = "is_zero_range")]
    pub angular_velocity: (f32, f32),
    /// Frames of an atlas each particle steps through, drawn from
    /// `texture`. `None` draws the whole texture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flipbook: Option<Flipbook>,
    /// The image every particle draws, by path. `None` is a flat square.
    #[serde(default)]
    pub texture: Option<AssetPath>,
    #[serde(skip)]
    /// What `texture` resolves to in this session's textures. Never written.
    pub texture_handle: Option<Handle<Texture>>,
    #[serde(default)]
    /// Draw order among sprites and tilemap layers. See [`Sprite::sort_order`](crate::Sprite::sort_order).
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
            shape: EmitShape::Point,
            size: (0.2, 0.0),
            size_curve: None,
            speed_curve: None,
            color_start: [1.0, 1.0, 1.0, 1.0],
            color_end: [1.0, 1.0, 1.0, 0.0],
            gradient: None,
            alpha_curve: None,
            rotation: (0.0, 0.0),
            angular_velocity: (0.0, 0.0),
            flipbook: None,
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
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "ParticleEmitter";

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

    /// What the GPU particles look up over their life: the curves, the
    /// gradient, the flipbook's frames resolved against `atlases` and the
    /// outline they are born on. See [`look::tables`].
    pub fn gpu_tables(
        &self,
        atlases: Option<&voltra_assets::Atlases>,
    ) -> voltra_render::ParticleTables {
        look::tables(self, atlases)
    }

    /// Refreshes the flipbook's atlas handle from its path, as
    /// [`Sprite::set_atlas`](crate::Sprite::set_atlas) does a sprite's.
    pub fn resolve_atlas(&mut self, atlases: &mut voltra_assets::Atlases) {
        if let Some(flipbook) = &mut self.flipbook {
            flipbook.resolve(atlases);
        }
    }

    /// Restarts it: every particle gone, the rate's debt and the bursts
    /// forgotten, the generator reseeded — what an editor's preview does
    /// after an edit. A GPU ring keeps its slots; its ledger empties, so its
    /// particles die out as new ones take their place.
    pub fn restart(&mut self) {
        self.state = Particles::default();
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

fn is_zero_range(range: &(f32, f32)) -> bool {
    *range == (0.0, 0.0)
}

#[cfg(test)]
mod collision_tests;
#[cfg(test)]
mod format_tests;
#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod tests;
