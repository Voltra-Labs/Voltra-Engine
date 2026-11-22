//! A light as the accumulate pass's vertex stage reads it.

use bytemuck::{Pod, Zeroable};

use super::PointLight;

/// One light as the vertex stage reads it. Must match `lights.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(super) struct LightInstance {
    /// centre.xy, radius, falloff
    place: [f32; 4],
    /// colour × intensity, height for normal maps (`0`: ignores them)
    color: [f32; 4],
    /// strength, softness, shadow map row (`-1` unshadowed), unused
    shadow: [f32; 4],
    /// direction.xy, cos of the outer half-angle, cos of the inner; a point
    /// light's are below any cosine, so every direction is inside
    cone: [f32; 4],
    /// the box it reaches, as fractions of its radius around its centre:
    /// min.xy, max.xy — the quad drawn, the square for a point light
    extent: [f32; 4],
    /// the light layers it lights
    layers: u32,
}

/// A point light's cone: both edges below -1, so every direction is in.
const EVERY_WAY: [f32; 4] = [1.0, 0.0, -2.0, -1.5];

impl PointLight {
    /// What the shader draws, or `None` for a light that gives nothing.
    /// `row` is the light's row in the shadow map; `None` draws it unshadowed.
    pub(super) fn instance(&self, row: Option<u32>) -> Option<LightInstance> {
        let reach = self.reach()?;
        let (min, max) = reach.local_bounds();
        let color = self.color.map(|c| c.max(0.0) * self.intensity);
        let shadow = match (row, self.casts()) {
            // Rows are counted in the thousands at most (the texture's height
            // over four), well inside what an `f32` holds exactly.
            (Some(row), Some((strength, softness))) => [strength, softness, row as f32, 0.0],
            _ => [0.0, 0.0, -1.0, 0.0],
        };
        let cone = self
            .cone
            .and_then(|cone| cone.sanitized())
            .map_or(EVERY_WAY, |(direction, outer, inner)| {
                [direction.x, direction.y, outer, inner]
            });
        Some(LightInstance {
            place: [
                self.position.x,
                self.position.y,
                self.radius,
                self.falloff.clamp(0.05, 16.0),
            ],
            color: [
                color[0],
                color[1],
                color[2],
                self.normal_height().unwrap_or(0.0),
            ],
            shadow,
            cone,
            extent: [min.x, min.y, max.x, max.y],
            layers: self.layers,
        })
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec2;

    use super::*;
    use crate::lights::{Cone, Shadow, MAX_SHADOW_SOFTNESS};

    fn lamp() -> PointLight {
        PointLight {
            position: Vec2::ZERO,
            color: [1.0, 0.5, 0.0],
            intensity: 2.0,
            radius: 3.0,
            falloff: 2.0,
            shadow: None,
            cone: None,
            height: None,
            layers: crate::lights::ALL_LIGHT_LAYERS,
        }
    }

    #[test]
    fn a_light_is_its_colour_times_its_intensity() {
        let i = lamp().instance(None).expect("gives light");
        assert_eq!(i.color[..3], [2.0, 1.0, 0.0]);
        assert_eq!(i.place, [0.0, 0.0, 3.0, 2.0]);
        assert_eq!(i.extent, [-1.0, -1.0, 1.0, 1.0], "a point light's square");
    }

    #[test]
    fn a_spot_light_draws_only_the_box_its_cone_reaches() {
        let spot = PointLight {
            cone: Some(Cone {
                direction: Vec2::NEG_X,
                angle: 0.5,
                softness: 0.0,
            }),
            ..lamp()
        };
        let [x0, y0, x1, y1] = spot.instance(None).expect("gives light").extent;
        assert_eq!(x0, -1.0, "out to the radius ahead");
        assert!(x1.abs() < 1e-6, "back to the centre, not behind it: {x1}");
        assert!(y0 > -0.3 && y1 < 0.3, "narrow across: {y0} {y1}");
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

    #[test]
    fn a_spot_light_carries_its_cone_and_a_point_light_shines_every_way() {
        assert_eq!(lamp().instance(None).expect("gives light").cone, EVERY_WAY);
        let spot = PointLight {
            cone: Some(Cone {
                direction: Vec2::new(0.0, 2.0),
                angle: std::f32::consts::FRAC_PI_2,
                softness: 0.5,
            }),
            ..lamp()
        };
        let cone = spot.instance(None).expect("gives light").cone;
        assert_eq!(cone[..2], [0.0, 1.0], "normalised");
        assert!((cone[2] - std::f32::consts::FRAC_PI_4.cos()).abs() < 1e-6);
        assert!((cone[3] - (std::f32::consts::FRAC_PI_8).cos()).abs() < 1e-6);
    }

    #[test]
    fn a_cone_that_shines_nowhere_is_a_light_that_gives_nothing() {
        for nowhere in [
            Cone {
                direction: Vec2::ZERO,
                angle: 1.0,
                softness: 0.0,
            },
            Cone {
                direction: Vec2::X,
                angle: 0.0,
                softness: 0.0,
            },
            Cone {
                direction: Vec2::X,
                angle: f32::NAN,
                softness: 0.0,
            },
        ] {
            let light = PointLight {
                cone: Some(nowhere),
                ..lamp()
            };
            assert_eq!(light.instance(None), None, "{nowhere:?}");
        }
        // A hard edge still has its two edges apart, and a full turn is
        // every way.
        let hard = Cone {
            direction: Vec2::X,
            angle: 1.0,
            softness: 0.0,
        };
        let (_, outer, inner) = hard.sanitized().expect("shines");
        assert!(inner > outer);
        let round = Cone {
            angle: 10.0,
            softness: 0.25,
            ..hard
        };
        let (_, outer, inner) = round.sanitized().expect("shines");
        assert!(
            outer < -1.0 && inner < -1.0,
            "both edges behind every direction"
        );
    }

    #[test]
    fn a_narrow_cone_is_fully_lit_down_its_middle() {
        let needle = Cone {
            direction: Vec2::X,
            angle: 1f32.to_radians(),
            softness: 0.0,
        };
        let (_, outer, inner) = needle.sanitized().expect("shines");
        assert!(inner <= 1.0 && outer < inner, "{outer} {inner}");
    }

    #[test]
    fn a_light_with_a_height_carries_it_and_nonsense_is_none() {
        assert_eq!(lamp().instance(None).expect("gives light").color[3], 0.0);
        let raised = PointLight {
            height: Some(1.5),
            ..lamp()
        };
        assert_eq!(raised.instance(None).expect("gives light").color[3], 1.5);
        for nonsense in [0.0, -1.0, f32::NAN] {
            let light = PointLight {
                height: Some(nonsense),
                ..lamp()
            };
            assert_eq!(light.instance(None).expect("gives light").color[3], 0.0);
        }
    }

    #[test]
    fn a_light_carries_its_layers() {
        let picky = PointLight {
            layers: 0b101,
            ..lamp()
        };
        assert_eq!(picky.instance(None).expect("gives light").layers, 0b101);
    }
}
