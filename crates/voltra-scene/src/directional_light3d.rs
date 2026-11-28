//! A light from infinitely far away: the sun.

use serde::{Deserialize, Serialize};

/// Lights the 3D world along its entity's −Z, every ray parallel:
/// Godot's `DirectionalLight3D`, Unity's directional `Light`.
///
/// One lights a view for now — with several, the one on the lowest entity
/// index, the rest warned about once (stage 139 lights them all).
///
/// `energy` is the light units' stored value (ARCHITECTURE "Light units"):
/// the illuminance in lux over K = 1.2 · 2¹⁵, so 1.0 — the default — is a
/// usable sun in simple units, and switching to physical units shows the
/// same number as some 39 000 lux without rewriting the file.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DirectionalLight3D {
    /// Linear RGB. White, the default, is not written.
    #[serde(default = "white", skip_serializing_if = "is_white")]
    pub color: [f32; 3],
    /// How bright, in the light units' energy. 1, the default, is not
    /// written.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub energy: f32,
}

fn white() -> [f32; 3] {
    [1.0; 3]
}

fn is_white(color: &[f32; 3]) -> bool {
    *color == white()
}

fn one() -> f32 {
    1.0
}

fn is_one(energy: &f32) -> bool {
    *energy == 1.0
}

impl Default for DirectionalLight3D {
    fn default() -> Self {
        Self {
            color: white(),
            energy: one(),
        }
    }
}

impl DirectionalLight3D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "DirectionalLight3D";

    /// What a value the light cannot draw with makes of it, if anything:
    /// shown by the inspector, warned about once by the frame.
    pub fn problem(&self) -> Option<&'static str> {
        if !self.color.iter().all(|c| c.is_finite() && *c >= 0.0) {
            Some("its colour is negative or not a number; it lights nothing")
        } else if !self.energy.is_finite() || self.energy < 0.0 {
            Some("its energy is negative or not a number; it lights nothing")
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_light_writes_nothing_and_reads_back() {
        let light = DirectionalLight3D::default();
        assert_eq!(ron::to_string(&light).expect("written"), "()");
        let warm = DirectionalLight3D {
            color: [1.0, 0.9, 0.7],
            energy: 2.5,
        };
        let text = ron::to_string(&warm).expect("written");
        assert_eq!(
            ron::from_str::<DirectionalLight3D>(&text).expect("read"),
            warm
        );
        assert!(warm.problem().is_none());
        let broken = DirectionalLight3D {
            energy: f32::NAN,
            ..warm
        };
        assert!(broken.problem().is_some());
    }
}
