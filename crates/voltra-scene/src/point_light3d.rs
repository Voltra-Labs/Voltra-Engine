//! A light at a point, shining every way.

use serde::{Deserialize, Serialize};

use crate::light::{all_light_layers, is_all_light_layers};
use crate::light3d::{is_one, is_white, one, white};

/// Lights the 3D world from its entity's position, every way, fading by
/// the inverse square of the distance to nothing at `range` — Godot's
/// `OmniLight3D`, Unity's point `Light`, Filament's point light.
///
/// `energy` is the light units' stored value (ARCHITECTURE "Light units"):
/// the luminous intensity in candela over K = 1.2 · 2¹⁵, so at 1 m an
/// energy of 1 lights as a `DirectionalLight3D` of energy 1 does, and a
/// quarter of that at 2 m. The range is in metres, not scaled with the
/// entity (as Unity's and Godot's are not). Each mesh is lit by the eight
/// point and spot lights that matter most to it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PointLight3D {
    /// Linear RGB. White, the default, is not written.
    #[serde(default = "white", skip_serializing_if = "is_white")]
    pub color: [f32; 3],
    /// How bright, in the light units' energy. 1, the default, is not
    /// written.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub energy: f32,
    /// Metres from it to where it gives nothing. 5, the default (Godot's),
    /// is not written.
    #[serde(default = "default_range", skip_serializing_if = "is_default_range")]
    pub range: f32,
    /// The light layers it lights: a mesh is lit by it only if its
    /// `light_layers` share a bit with these. Every bit — the default, and
    /// not written — lights everything.
    #[serde(
        default = "all_light_layers",
        skip_serializing_if = "is_all_light_layers"
    )]
    pub layers: u32,
}

/// A point or spot light's range when nothing says otherwise: 5 m,
/// Godot's.
pub(crate) fn default_range() -> f32 {
    PointLight3D::DEFAULT_RANGE
}

pub(crate) fn is_default_range(range: &f32) -> bool {
    *range == PointLight3D::DEFAULT_RANGE
}

impl Default for PointLight3D {
    fn default() -> Self {
        Self {
            color: white(),
            energy: one(),
            range: default_range(),
            layers: all_light_layers(),
        }
    }
}

impl PointLight3D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "PointLight3D";

    /// The range a light has unless told otherwise: 5 m, Godot's.
    pub const DEFAULT_RANGE: f32 = 5.0;

    /// What a value the light cannot draw with makes of it, if anything:
    /// shown by the inspector, warned about once by the frame.
    pub fn problem(&self) -> Option<&'static str> {
        crate::light3d::problem(&self.color, self.energy)
            .or_else(|| crate::light3d::range_problem(self.range))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_point_light_writes_nothing_and_reads_back() {
        let light = PointLight3D::default();
        assert_eq!(ron::to_string(&light).expect("written"), "()");
        let warm = PointLight3D {
            color: [1.0, 0.7, 0.4],
            energy: 3.0,
            range: 8.0,
            layers: 0b10,
        };
        let text = ron::to_string(&warm).expect("written");
        assert_eq!(ron::from_str::<PointLight3D>(&text).expect("read"), warm);
        assert!(warm.problem().is_none());
        for broken in [
            PointLight3D {
                range: -1.0,
                ..warm
            },
            PointLight3D {
                range: f32::NAN,
                ..warm
            },
            PointLight3D {
                energy: f32::INFINITY,
                ..warm
            },
            PointLight3D {
                color: [-1.0, 0.0, 0.0],
                ..warm
            },
        ] {
            assert!(broken.problem().is_some(), "{broken:?}");
        }
    }
}
