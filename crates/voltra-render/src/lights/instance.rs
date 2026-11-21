//! A light as the accumulate pass's vertex stage reads it.

use bytemuck::{Pod, Zeroable};

use super::PointLight;

/// One light as the vertex stage reads it. Must match `lights.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(super) struct LightInstance {
    /// centre.xy, radius, falloff
    place: [f32; 4],
    /// colour × intensity, unused
    color: [f32; 4],
    /// strength, softness, shadow map row (`-1` unshadowed), unused
    shadow: [f32; 4],
}

impl PointLight {
    /// What the shader draws, or `None` for a light that gives nothing.
    /// `row` is the light's row in the shadow map; `None` draws it unshadowed.
    pub(super) fn instance(&self, row: Option<u32>) -> Option<LightInstance> {
        if !self.gives_light() {
            return None;
        }
        let color = self.color.map(|c| c.max(0.0) * self.intensity);
        let shadow = match (row, self.casts()) {
            // Rows are counted in the thousands at most (the texture's height
            // over four), well inside what an `f32` holds exactly.
            (Some(row), Some((strength, softness))) => [strength, softness, row as f32, 0.0],
            _ => [0.0, 0.0, -1.0, 0.0],
        };
        Some(LightInstance {
            place: [
                self.position.x,
                self.position.y,
                self.radius,
                self.falloff.clamp(0.05, 16.0),
            ],
            color: [color[0], color[1], color[2], 0.0],
            shadow,
        })
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec2;

    use super::*;
    use crate::lights::{Shadow, MAX_SHADOW_SOFTNESS};

    fn lamp() -> PointLight {
        PointLight {
            position: Vec2::ZERO,
            color: [1.0, 0.5, 0.0],
            intensity: 2.0,
            radius: 3.0,
            falloff: 2.0,
            shadow: None,
        }
    }

    #[test]
    fn a_light_is_its_colour_times_its_intensity() {
        let i = lamp().instance(None).expect("gives light");
        assert_eq!(i.color[..3], [2.0, 1.0, 0.0]);
        assert_eq!(i.place, [0.0, 0.0, 3.0, 2.0]);
    }

    #[test]
    fn a_light_that_gives_nothing_is_not_drawn() {
        for dark in [
            PointLight {
                radius: 0.0,
                ..lamp()
            },
            PointLight {
                intensity: 0.0,
                ..lamp()
            },
            PointLight {
                color: [0.0; 3],
                ..lamp()
            },
            PointLight {
                position: Vec2::NAN,
                ..lamp()
            },
        ] {
            assert_eq!(dark.instance(None), None, "{dark:?}");
        }
    }

    #[test]
    fn a_shadow_is_drawn_only_with_a_row_and_a_strength() {
        let shadowed = PointLight {
            shadow: Some(Shadow {
                strength: 2.0,
                softness: 1.0,
            }),
            ..lamp()
        };
        let i = shadowed.instance(Some(3)).expect("gives light");
        assert_eq!(i.shadow, [1.0, MAX_SHADOW_SOFTNESS, 3.0, 0.0], "clamped");
        // No row to draw into: lit as if nothing were in the way.
        assert_eq!(
            shadowed.instance(None).expect("gives light").shadow[2],
            -1.0
        );
        // A row but no shadow asked for.
        assert_eq!(
            lamp().instance(Some(0)).expect("gives light").shadow[2],
            -1.0
        );
    }
}
