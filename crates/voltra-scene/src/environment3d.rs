//! What surrounds the 3D world: its ambient light, the background drawn
//! where nothing else is, and the fog.

use serde::{Deserialize, Serialize};
use voltra_render::render3d::{Fog, SkyGradient};

/// The 3D world's surroundings: Godot's `WorldEnvironment`. On a camera's
/// entity it is that camera's; on any other entity it is the world's, used
/// by every camera without its own — with several, the one on the lowest
/// entity index, the rest warned about once. With none, the default.
///
/// Stage 141 adds a sky that lights (a panorama, and an ambient taken from
/// it).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Environment3D {
    /// A uniform sky's light, linear RGB in the light units' energy: the
    /// diffuse it gives a white surface, before occlusion. Lights a surface
    /// facing away from every light. The default, a dim grey of 0.1, is not
    /// written.
    #[serde(default = "dim", skip_serializing_if = "is_dim")]
    pub ambient: [f32; 3],
    /// What `ambient` is multiplied by — Godot's `ambient_light_energy`. 1,
    /// the default, is not written.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub ambient_energy: f32,
    /// What is drawn where no mesh is. `Clear`, the default and not
    /// written, is the camera's clear colour, as before there was a sky.
    #[serde(default, skip_serializing_if = "Background3D::is_clear")]
    pub background: Background3D,
    /// Fog between the eye and what it sees; `None`, the default and not
    /// written, is clear air.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fog: Option<Fog3D>,
}

/// What a 3D camera shows where no mesh is drawn: Godot's environment
/// background modes. Colours are linear RGB in the light units' energy (a
/// luminance over K), so past 1 is brighter than the default exposure's
/// white.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum Background3D {
    /// The camera's clear colour: the renderer's background, or the
    /// camera's own `clear`.
    #[default]
    Clear,
    /// One colour every way.
    Color([f32; 3]),
    /// A sky of three colours by elevation, blended linearly in the angle:
    /// `top` straight up, `horizon` level, `bottom` straight down.
    Gradient {
        /// Straight up.
        top: [f32; 3],
        /// Level with the eye, every way round.
        horizon: [f32; 3],
        /// Straight down.
        bottom: [f32; 3],
    },
}

impl Background3D {
    /// A daylight sky, Godot's procedural sky's colours: what the inspector
    /// starts a gradient from.
    pub const DAYLIGHT: Self = Self::Gradient {
        top: [0.385, 0.454, 0.55],
        horizon: [0.646, 0.656, 0.67],
        bottom: [0.2, 0.169, 0.133],
    };

    fn is_clear(&self) -> bool {
        *self == Self::Clear
    }

    /// The sky the renderer draws, if one is drawn: the gradient's.
    pub fn sky(&self) -> Option<SkyGradient> {
        match *self {
            Self::Gradient {
                top,
                horizon,
                bottom,
            } => Some(SkyGradient {
                top,
                horizon,
                bottom,
            }),
            Self::Clear | Self::Color(_) => None,
        }
    }

    /// What a colour of it the renderer cannot draw makes of it, if
    /// anything: drawn as black there.
    pub fn problem(&self) -> Option<&'static str> {
        let bad = |c: &[f32; 3]| !c.iter().all(|c| c.is_finite() && *c >= 0.0);
        let broken = match self {
            Self::Clear => false,
            Self::Color(c) => bad(c),
            Self::Gradient {
                top,
                horizon,
                bottom,
            } => bad(top) || bad(horizon) || bad(bottom),
        };
        broken.then_some("a background colour is negative or not a number; drawn black there")
    }
}

/// Fog: one medium filling the world, denser below `height` — Filament's
/// height fog, integrated exactly from the eye to each point (see
/// [`Fog`](voltra_render::render3d::Fog)). `height_falloff` 0 is plain
/// distance fog; `sky_tint` lets the sky behind a distant thing colour the
/// fog it fades into (Godot's aerial perspective).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Fog3D {
    /// What a distant thing fades to, linear RGB in the light units'
    /// energy. Godot's default fog colour.
    pub color: [f32; 3],
    /// How thick it is at `height`, per metre: at 0.01, the default
    /// (Godot's), half the light is gone in some 69 m.
    pub density: f32,
    /// The height, in metres, at which it is `density` thick.
    pub height: f32,
    /// How fast it thins with height, per metre: 0, the default, the same
    /// everywhere.
    pub height_falloff: f32,
    /// How much of the sky's colour, along the way the eye looks, replaces
    /// `color`: 0 (the default) none, 1 all — distant things fade into the
    /// sky behind them.
    pub sky_tint: f32,
}

impl Default for Fog3D {
    fn default() -> Self {
        Self {
            color: [0.518, 0.553, 0.608],
            density: 0.01,
            height: 0.0,
            height_falloff: 0.0,
            sky_tint: 0.0,
        }
    }
}

impl Fog3D {
    /// The fog the renderer draws.
    pub fn fog(&self) -> Fog {
        Fog {
            color: self.color,
            density: self.density,
            height: self.height,
            height_falloff: self.height_falloff,
            sky_tint: self.sky_tint,
        }
    }

    /// What a number of it the renderer cannot draw makes of it, if
    /// anything: drawn as none of that number.
    pub fn problem(&self) -> Option<&'static str> {
        let all = [
            self.color[0],
            self.color[1],
            self.color[2],
            self.density,
            self.height_falloff,
            self.sky_tint,
        ];
        (!all.iter().all(|v| v.is_finite() && *v >= 0.0) || !self.height.is_finite())
            .then_some("a fog number is negative or not a number; drawn as 0")
    }
}

fn dim() -> [f32; 3] {
    [Environment3D::DEFAULT_AMBIENT; 3]
}

fn is_dim(ambient: &[f32; 3]) -> bool {
    *ambient == dim()
}

fn one() -> f32 {
    1.0
}

fn is_one(v: &f32) -> bool {
    *v == 1.0
}

impl Default for Environment3D {
    fn default() -> Self {
        Self {
            ambient: dim(),
            ambient_energy: one(),
            background: Background3D::Clear,
            fog: None,
        }
    }
}

impl Environment3D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Environment3D";

    /// The ambient of a world with no environment: a dim grey, so a mesh
    /// with no light is still seen (Godot's default environment lights by
    /// its clear colour, Bevy's `AmbientLight` by a dim white); an
    /// `Environment3D` with black ambient makes the dark.
    pub const DEFAULT_AMBIENT: f32 = 0.1;

    /// An environment of `ambient`, the rest the default's.
    pub fn with_ambient(ambient: [f32; 3]) -> Self {
        Self {
            ambient,
            ..Self::default()
        }
    }

    /// The ambient the renderer lights with: the colour times its energy.
    pub fn ambient_light(&self) -> [f32; 3] {
        self.ambient.map(|c| c * self.ambient_energy)
    }

    /// What a number of it the renderer cannot draw makes of it, if
    /// anything: the inspector shows it, the frame warns about it once.
    pub fn problem(&self) -> Option<&'static str> {
        let ambient = self.ambient_light();
        if !ambient.iter().all(|c| c.is_finite() && *c >= 0.0) {
            return Some("the ambient is negative or not a number; drawn as none");
        }
        self.background
            .problem()
            .or_else(|| self.fog.and_then(|fog| fog.problem()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_environment_writes_nothing_and_reads_back() {
        let environment = Environment3D::default();
        assert_eq!(ron::to_string(&environment).expect("written"), "()");
        // A stage-137 file: the ambient alone.
        let old: Environment3D = ron::from_str("(ambient: (0.0, 0.0, 0.0))").expect("read");
        assert_eq!(old, Environment3D::with_ambient([0.0; 3]));
        let foggy = Environment3D {
            ambient_energy: 2.0,
            background: Background3D::DAYLIGHT,
            fog: Some(Fog3D {
                sky_tint: 1.0,
                ..Fog3D::default()
            }),
            ..Environment3D::default()
        };
        let text = ron::to_string(&foggy).expect("written");
        assert_eq!(ron::from_str::<Environment3D>(&text).expect("read"), foggy);
        assert_eq!(foggy.ambient_light(), [0.2; 3]);
        assert!(foggy.problem().is_none());
        assert!(foggy.background.sky().is_some());
        assert_eq!(Background3D::Color([1.0; 3]).sky(), None);
        let broken = Environment3D {
            fog: Some(Fog3D {
                density: f32::NAN,
                ..Fog3D::default()
            }),
            ..Environment3D::default()
        };
        assert!(broken.problem().is_some());
        let dark_sky = Environment3D {
            background: Background3D::Color([-1.0, 0.0, 0.0]),
            ..Environment3D::default()
        };
        assert!(dark_sky.problem().is_some());
    }
}
