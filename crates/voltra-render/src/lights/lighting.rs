//! What a frame is lit by: the ambient, the point lights and what blocks
//! them.

use glam::Vec2;

use super::Occluder;

/// The light map's and the lit frame's format: linear, with room past one.
pub const LIT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The shadow map's width when nothing asks for another: texels across each
/// quarter turn around a light. Godot's 2D shadow atlas defaults to 2048
/// across a whole turn's four rows packed differently; 1024 a quarter is a
/// sharper edge than that at the same cost per light.
pub const DEFAULT_SHADOW_RESOLUTION: u32 = 1024;

/// The widest penumbra a light can ask for, in radians either side of the
/// edge (about 5.7°). The shader takes thirteen point taps across it; wider,
/// they would sit far enough apart for the fade to show as steps.
pub const MAX_SHADOW_SOFTNESS: f32 = 0.1;

/// A light shining from a point, fading to nothing at its radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointLight {
    /// Where, in the world.
    pub position: Vec2,
    /// Linear colour, multiplied by `intensity`.
    pub color: [f32; 3],
    pub intensity: f32,
    /// World units from the centre to where it gives nothing.
    pub radius: f32,
    /// How the light fades with distance: `1` is a straight ramp, `2` the
    /// soft default; higher keeps the light near the centre.
    pub falloff: f32,
    /// Whether the frame's [`Occluder`]s block it, and how much. `None`
    /// passes through everything, and costs no shadow map.
    pub shadow: Option<Shadow>,
    /// A spot light's cone; `None` shines every way.
    pub cone: Option<Cone>,
}

/// The way a spot light shines: a cone around `direction`, Unity URP's
/// spot `Light2D`. Outside it, nothing; inside, the light as a point light
/// would give it; between, a fade.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cone {
    /// Where the cone points, in the world. Its length does not matter.
    pub direction: Vec2,
    /// The whole angle across the cone, in radians. A full turn or more is
    /// a point light.
    pub angle: f32,
    /// How much of the cone's half-angle, from its edge inward, fades: `0`
    /// is a hard edge, `1` fades from the middle out.
    pub softness: f32,
}

impl Cone {
    /// `(direction, cos of the outer half-angle, cos of the inner)` for the
    /// shader, the inner strictly above the outer; `None` for a cone that
    /// shines nowhere (no angle, no direction, not a number).
    pub(super) fn sanitized(&self) -> Option<(Vec2, f32, f32)> {
        let finite =
            self.direction.is_finite() && self.angle.is_finite() && self.softness.is_finite();
        let direction = self.direction.try_normalize()?;
        if !finite || self.angle <= 0.0 {
            return None;
        }
        let half = self.angle * 0.5;
        if half >= std::f32::consts::PI {
            // A full turn: every way, as a point light — edges below any
            // cosine, rather than a fade behind it.
            return Some((direction, -2.0, -1.5));
        }
        let inner = (half * (1.0 - self.softness.clamp(0.0, 1.0))).cos();
        // `smoothstep` needs its edges apart; a hard edge is a very short
        // fade, opened outward so the inner edge stays at most 1 and the
        // middle of a narrow cone is still fully lit.
        let outer = half.cos().min(inner - 1e-4);
        Some((direction, outer, inner))
    }
}

/// How a light's shadows look.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    /// How much of the light a shadow takes away: `1` all of it, `0` none.
    pub strength: f32,
    /// How far either side of a shadow's edge it fades, in radians around
    /// the light: `0` is a hard edge. At most [`MAX_SHADOW_SOFTNESS`].
    pub softness: f32,
}

impl Shadow {
    /// `(strength, softness)` clamped to what the shader takes, or `None`
    /// for a shadow that takes nothing away or is not a number.
    pub(super) fn sanitized(&self) -> Option<(f32, f32)> {
        let finite = self.strength.is_finite() && self.softness.is_finite();
        let strength = self.strength.clamp(0.0, 1.0);
        (finite && strength > 0.0)
            .then(|| (strength, self.softness.clamp(0.0, MAX_SHADOW_SOFTNESS)))
    }
}

impl PointLight {
    /// Whether the light gives anything: a radius, an intensity, a colour,
    /// a cone that shines somewhere if it has one, every value a number.
    /// What the renderer draws, and what an editor outline should agree
    /// with.
    pub fn gives_light(&self) -> bool {
        let finite = self.position.is_finite()
            && self.color.iter().all(|c| c.is_finite())
            && self.intensity.is_finite()
            && self.radius.is_finite()
            && self.falloff.is_finite();
        finite
            && self.radius > 0.0
            && self.intensity > 0.0
            && self.color.iter().any(|&c| c > 0.0)
            && self.cone.is_none_or(|cone| cone.sanitized().is_some())
    }

    /// The shadow it casts, sanitised, if it gives light and casts one.
    pub(super) fn casts(&self) -> Option<(f32, f32)> {
        self.shadow
            .and_then(|shadow| shadow.sanitized())
            .filter(|_| self.gives_light())
    }
}

/// A frame's lighting: unlit without an ambient colour, lit by it and the
/// lights with one, which the occluders shadow.
#[derive(Debug, Clone, PartialEq)]
pub struct Lighting {
    /// The light everything gets, linear. `None` is unlit: the frame is the
    /// scene as drawn and no lighting pass runs.
    pub ambient: Option<[f32; 3]>,
    pub lights: Vec<PointLight>,
    /// What blocks the lights that cast shadows, in the world.
    pub occluders: Vec<Occluder>,
    /// Shadow map texels across each quarter turn around a light; clamped to
    /// what the device allows.
    pub shadow_resolution: u32,
}

impl Default for Lighting {
    /// Unlit, with nothing in it.
    fn default() -> Self {
        Self {
            ambient: None,
            lights: Vec::new(),
            occluders: Vec::new(),
            shadow_resolution: DEFAULT_SHADOW_RESOLUTION,
        }
    }
}

impl Lighting {
    /// The ambient colour, a non-number or negative channel as zero; `None`
    /// when unlit.
    pub(super) fn ambient(&self) -> Option<wgpu::Color> {
        let channel = |c: f32| {
            if c.is_finite() {
                f64::from(c.max(0.0))
            } else {
                0.0
            }
        };
        self.ambient.map(|[r, g, b]| wgpu::Color {
            r: channel(r),
            g: channel(g),
            b: channel(b),
            a: 1.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lamp() -> PointLight {
        PointLight {
            position: Vec2::ZERO,
            color: [1.0, 0.5, 0.0],
            intensity: 2.0,
            radius: 3.0,
            falloff: 2.0,
            shadow: None,
            cone: None,
        }
    }

    #[test]
    fn unlit_is_no_ambient_and_nonsense_ambient_is_black() {
        assert_eq!(Lighting::default().ambient(), None);
        let odd = Lighting {
            ambient: Some([f32::NAN, -1.0, 0.5]),
            ..Lighting::default()
        };
        let c = odd.ambient().expect("lit");
        assert_eq!((c.r, c.g, c.b), (0.0, 0.0, 0.5));
    }

    #[test]
    fn a_shadow_that_takes_nothing_away_is_no_shadow() {
        for nothing in [
            Shadow {
                strength: 0.0,
                softness: 0.1,
            },
            Shadow {
                strength: f32::NAN,
                softness: 0.1,
            },
            Shadow {
                strength: 1.0,
                softness: f32::INFINITY,
            },
        ] {
            assert_eq!(nothing.sanitized(), None, "{nothing:?}");
        }
        let dark_lamp = PointLight {
            intensity: 0.0,
            shadow: Some(Shadow {
                strength: 1.0,
                softness: 0.0,
            }),
            ..lamp()
        };
        assert_eq!(
            dark_lamp.casts(),
            None,
            "a light that gives nothing casts nothing"
        );
    }
}
