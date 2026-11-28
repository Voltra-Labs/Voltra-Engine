//! A light at a point, shining in a cone.

use serde::{Deserialize, Serialize};

use crate::light::{all_light_layers, is_all_light_layers};
use crate::light3d::{is_one, is_white, one, white};
use crate::point_light3d::{default_range, is_default_range};

/// Lights the 3D world from its entity's position down its −Z, in a cone:
/// whole within `inner_angle` of the axis, fading to nothing at
/// `outer_angle` — Godot's `SpotLight3D`, glTF's `KHR_lights_punctual`
/// spot (whose cone angles these are), Filament's spot light.
///
/// Like a [`PointLight3D`](crate::PointLight3D), it fades by the inverse
/// square to nothing at `range` (metres, not scaled), and its `energy` is
/// the intensity in candela over K — the same within the cone however
/// wide it is (Filament's decoupled spot), so widening it lights more, not
/// dimmer. Each mesh is lit by the eight point and spot lights that matter
/// most to it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpotLight3D {
    /// Linear RGB. White, the default, is not written.
    #[serde(default = "white", skip_serializing_if = "is_white")]
    pub color: [f32; 3],
    /// How bright, in the light units' energy. 1, the default, is not
    /// written.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub energy: f32,
    /// Metres from it to where it gives nothing. 5, the default, is not
    /// written.
    #[serde(default = "default_range", skip_serializing_if = "is_default_range")]
    pub range: f32,
    /// Degrees from the axis within which it shines whole. 30, the default,
    /// is not written; past `outer_angle` it is `outer_angle` (a hard
    /// edge).
    #[serde(default = "default_inner", skip_serializing_if = "is_default_inner")]
    pub inner_angle: f32,
    /// Degrees from the axis past which it gives nothing, up to 90 (a
    /// half-turn across). 45, the default (Godot's), is not written.
    #[serde(default = "default_outer", skip_serializing_if = "is_default_outer")]
    pub outer_angle: f32,
    /// The light layers it lights; every bit — the default, and not
    /// written — lights everything.
    #[serde(
        default = "all_light_layers",
        skip_serializing_if = "is_all_light_layers"
    )]
    pub layers: u32,
}

fn default_inner() -> f32 {
    SpotLight3D::DEFAULT_INNER_ANGLE
}

fn is_default_inner(angle: &f32) -> bool {
    *angle == SpotLight3D::DEFAULT_INNER_ANGLE
}

fn default_outer() -> f32 {
    SpotLight3D::DEFAULT_OUTER_ANGLE
}

fn is_default_outer(angle: &f32) -> bool {
    *angle == SpotLight3D::DEFAULT_OUTER_ANGLE
}

impl Default for SpotLight3D {
    fn default() -> Self {
        Self {
            color: white(),
            energy: one(),
            range: default_range(),
            inner_angle: default_inner(),
            outer_angle: default_outer(),
            layers: all_light_layers(),
        }
    }
}

impl SpotLight3D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "SpotLight3D";

    /// The inner angle unless told otherwise, in degrees from the axis.
    pub const DEFAULT_INNER_ANGLE: f32 = 30.0;

    /// The outer angle unless told otherwise, in degrees from the axis:
    /// Godot's 45.
    pub const DEFAULT_OUTER_ANGLE: f32 = 45.0;

    /// The widest the cone opens, in degrees from the axis.
    pub const MAX_ANGLE: f32 = 90.0;

    /// What a value the light cannot draw with makes of it, if anything:
    /// shown by the inspector, warned about once by the frame. An angle
    /// past 90° or an inner past the outer is drawn clamped, not refused.
    pub fn problem(&self) -> Option<&'static str> {
        crate::light3d::problem(&self.color, self.energy)
            .or_else(|| crate::light3d::range_problem(self.range))
            .or_else(|| {
                (!self.inner_angle.is_finite() || !self.outer_angle.is_finite())
                    .then_some("a cone angle is not a number; it lights nothing")
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_spot_light_writes_nothing_and_reads_back() {
        let light = SpotLight3D::default();
        assert_eq!(ron::to_string(&light).expect("written"), "()");
        let narrow = SpotLight3D {
            inner_angle: 10.0,
            outer_angle: 15.0,
            energy: 4.0,
            ..light
        };
        let text = ron::to_string(&narrow).expect("written");
        assert_eq!(ron::from_str::<SpotLight3D>(&text).expect("read"), narrow);
        assert!(narrow.problem().is_none());
        let broken = SpotLight3D {
            outer_angle: f32::NAN,
            ..narrow
        };
        assert!(broken.problem().is_some());
    }
}
