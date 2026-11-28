//! What lights a 3D frame: directional lights, point and spot lights, the
//! ambient, the sky behind and the fog between — and the uniforms the
//! surface and sky shaders read them through.
//!
//! **Forward, with a light list per instance** (ARCHITECTURE "Lights,
//! shadows and the environment"): the frame's lights go into one uniform
//! array ([`LightsUniform3D`]: up to [`MAX_DIRECTIONAL`] directional and
//! [`MAX_PUNCTUAL`] point and spot lights), and each instance carries the
//! indices of the [`LIGHTS_PER_INSTANCE`] point and spot lights that matter
//! most to it ([`LightList3D`]), chosen on the CPU by the scene side —
//! Godot's Mobile and Compatibility renderers' model, since WebGL2 has no
//! storage buffers for a clustered grid.

mod environment;
mod falloff;
mod frame_uniform;
mod lights_uniform;
mod list;

pub use environment::{Fog, SkyGradient};
pub use falloff::{distance_attenuation, SpotCone, MIN_DISTANCE_SQUARED};
pub use frame_uniform::FrameUniform3D;
pub use lights_uniform::LightsUniform3D;
pub use list::LightList3D;

use glam::Vec3;

/// The most directional lights a view is lit by.
pub const MAX_DIRECTIONAL: usize = 4;

/// The most point and spot lights a view holds: the uniform array's
/// length, which an instance's list indexes.
pub const MAX_PUNCTUAL: usize = 128;

/// The most point and spot lights one instance is lit by: eight `u8`
/// indices, two `u32`s of its instance attribute.
pub const LIGHTS_PER_INSTANCE: usize = 8;

/// A light from infinitely far away, all its rays parallel: the sun.
///
/// `color × energy` is what the shader lights with. `energy` is the light
/// units' stored value (ARCHITECTURE "Light units"): the illuminance in lux
/// over K = 1.2 · 2¹⁵, which an exposure of EV100 15 maps to 1 — so 1.0 is
/// a usable light in simple units, and physical units (stage 142) read the
/// same number as lux through K.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirectionalLight {
    /// The way the light travels, in the world: its entity's −Z.
    pub direction: Vec3,
    /// Linear RGB.
    pub color: [f32; 3],
    /// The light units' stored value.
    pub energy: f32,
}

/// A light at a point, shining every way — or, with a [`SpotCone`], in a
/// cone — fading by the inverse square of the distance to nothing at
/// `range` ([`distance_attenuation`]).
///
/// `energy` is the light units' stored value: the luminous intensity in
/// candela over K, so `color × energy / d²` is the illuminance it gives at
/// `d` metres in the units a [`DirectionalLight`]'s energy is in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PunctualLight {
    /// Where it is, in the world.
    pub position: Vec3,
    /// Linear RGB.
    pub color: [f32; 3],
    /// The light units' stored value.
    pub energy: f32,
    /// Metres from it to where it gives nothing.
    pub range: f32,
    /// A spot light's cone; `None` for a point light.
    pub spot: Option<SpotCone>,
}

impl PunctualLight {
    /// The light it gives a point `to_light` away from it (the vector from
    /// the lit point to the light), per unit colour: the distance window
    /// and, for a spot light, the cone. `surface3d_light.wgsl`'s
    /// `punctual_attenuation`.
    pub fn attenuation(&self, to_light: Vec3) -> f32 {
        let distance = distance_attenuation(to_light.length_squared(), self.range);
        let cone = self
            .spot
            .map_or(1.0, |spot| spot.attenuation(-to_light.normalize_or_zero()));
        distance * cone
    }

    /// Whether a number of it cannot be drawn with: not finite, negative.
    /// Such a light is left out of the uniform (it lights nothing).
    pub fn is_broken(&self) -> bool {
        !(self.position.is_finite()
            && self.color.iter().all(|c| c.is_finite() && *c >= 0.0)
            && self.energy.is_finite()
            && self.energy >= 0.0
            && self.range.is_finite()
            && self.range >= 0.0)
    }
}

/// A 3D frame's light: what [`Scene3D`](super::Scene3D) is lit by.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Lighting3D {
    /// The directional lights, the first [`MAX_DIRECTIONAL`] used; an
    /// instance's [`LightList3D`] says which reach it.
    pub directional: Vec<DirectionalLight>,
    /// The point and spot lights an instance's [`LightList3D`] indexes,
    /// the first [`MAX_PUNCTUAL`] used.
    pub punctual: Vec<PunctualLight>,
    /// A uniform sky's light, linear RGB in the light units' energy: the
    /// diffuse it gives a white surface.
    pub ambient: [f32; 3],
    /// The sky drawn where nothing else is; `None` leaves the clear colour
    /// there.
    pub sky: Option<SkyGradient>,
    /// The fog between the eye and what it sees; `None` for clear air.
    pub fog: Option<Fog>,
}

impl Lighting3D {
    /// No light at all: an unlit surface still draws as it is.
    pub const DARK: Self = Self {
        directional: Vec::new(),
        punctual: Vec::new(),
        ambient: [0.0; 3],
        sky: None,
        fog: None,
    };

    /// One sun and an ambient: what stage 137 lit a frame with.
    pub fn sun(light: DirectionalLight, ambient: [f32; 3]) -> Self {
        Self {
            directional: vec![light],
            ambient,
            ..Self::DARK
        }
    }
}
