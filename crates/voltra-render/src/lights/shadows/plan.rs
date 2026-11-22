//! Which lights get rows in the shadow map, and the casting edges each
//! draws there.

use super::QUARTERS;
use crate::lights::occluder::Edges;
use crate::lights::PointLight;

/// One edge drawn into one light's rows. Must match `light_shadows.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(in crate::lights) struct ShadowInstance {
    /// a.xy, b.xy, in the world
    ends: [f32; 4],
    /// centre.xy, radius, top of the light's first row in clip space
    light: [f32; 4],
    /// one row's height in clip space
    row_height: f32,
}

/// What a frame draws into its shadow map, and which row each light got.
#[derive(Debug, Clone, Default, PartialEq)]
pub(in crate::lights) struct ShadowPlan {
    /// Per light of the frame, in order: its row, or `None` for a light
    /// that is drawn unshadowed — it casts none, nothing it reaches (inside
    /// its cone, for a spot light) occludes, or the map ran out of rows.
    pub(in crate::lights) rows: Vec<Option<u32>>,
    pub(in crate::lights) instances: Vec<ShadowInstance>,
    /// Texels across a row.
    pub(in crate::lights) width: u32,
    /// Rows in the texture: what was used, rounded up to a power of two so
    /// a light coming or going does not reallocate it every frame.
    pub(in crate::lights) height: u32,
    /// Shadowed lights left unshadowed for want of rows.
    pub(in crate::lights) overflow: usize,
}

impl ShadowPlan {
    /// Rows for the `lights` whose shadows something in `edges` casts,
    /// `resolution` texels across, in a texture at most `max_dimension` on
    /// a side.
    pub(in crate::lights) fn new(
        lights: &[PointLight],
        edges: &Edges,
        resolution: u32,
        max_dimension: u32,
    ) -> Self {
        let max_rows = max_dimension / QUARTERS;
        let mut plan = Self {
            rows: vec![None; lights.len()],
            width: resolution.clamp(1, max_dimension.max(1)),
            ..Self::default()
        };
        if edges.is_empty() {
            return plan;
        }
        // Each instance knows its light's row; the texture's height, which
        // turns a row into clip space, is only known once every light has
        // one, so the band is filled in after.
        let mut used = 0u32;
        let mut rows_of = Vec::new();
        for (index, light) in lights.iter().enumerate() {
            let (Some((_, softness)), Some(reach)) = (light.casts(), light.reach()) else {
                continue;
            };
            // Only what comes into a spot light's cone shadows it — opened by
            // the soft edge, whose taps look that far past it.
            let reach = reach.widened(softness);
            let first = plan.instances.len();
            for (a, b) in edges
                .casting(light.position, light.radius, light.layers)
                .filter(|(a, b)| reach.touches_segment(*a, *b))
            {
                plan.instances.push(ShadowInstance {
                    ends: [a.x, a.y, b.x, b.y],
                    light: [light.position.x, light.position.y, light.radius, 0.0],
                    row_height: 0.0,
                });
            }
            if plan.instances.len() == first {
                continue;
            }
            if used == max_rows {
                plan.instances.truncate(first);
                plan.overflow += 1;
                continue;
            }
            plan.rows[index] = Some(used);
            rows_of.push((first..plan.instances.len(), used));
            used += 1;
        }
        if used == 0 {
            return plan;
        }
        let rows = used.next_power_of_two().min(max_rows);
        plan.height = rows * QUARTERS;
        // Clip space runs from 1 at the texture's top row to -1 at its
        // bottom; a light's first row starts 4·row texel rows down.
        let row_height = 2.0 / plan.height as f32;
        for (range, row) in rows_of {
            let top = 1.0 - (row * QUARTERS) as f32 * row_height;
            for instance in &mut plan.instances[range] {
                instance.light[3] = top;
                instance.row_height = row_height;
            }
        }
        plan
    }
}

/// The plan's instance count, as a draw takes it.
pub(in crate::lights) fn instance_count(plan: &ShadowPlan) -> u32 {
    u32::try_from(plan.instances.len()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use glam::Vec2;

    use super::*;
    use crate::lights::{Cone, Occluder, Shadow};

    fn lamp(at: Vec2, shadow: bool) -> PointLight {
        PointLight {
            position: at,
            color: [1.0; 3],
            intensity: 1.0,
            radius: 5.0,
            falloff: 2.0,
            shadow: shadow.then_some(Shadow {
                strength: 1.0,
                softness: 0.0,
            }),
            cone: None,
            height: None,
            layers: crate::lights::ALL_LIGHT_LAYERS,
        }
    }

    /// Where a light's `row` starts: its first row's top, in clip space.
    fn top_of(plan: &ShadowPlan, row: u32) -> f32 {
        1.0 - (row * QUARTERS) as f32 * 2.0 / plan.height as f32
    }

    fn wall_at(x: f32) -> Occluder {
        Occluder {
            points: vec![Vec2::new(x, -1.0), Vec2::new(x, 1.0)],
            closed: false,
            self_shadow: false,
            layers: crate::lights::ALL_LIGHT_LAYERS,
        }
    }

    #[test]
    fn only_shadowed_lights_with_an_edge_in_reach_get_a_row() {
        let edges = Edges::new(&[wall_at(2.0)]);
        let lights = [
            lamp(Vec2::ZERO, true),
            lamp(Vec2::ZERO, false),
            // Shadowed, but the wall is out of its reach.
            lamp(Vec2::new(-20.0, 0.0), true),
            lamp(Vec2::new(4.0, 0.0), true),
        ];
        let plan = ShadowPlan::new(&lights, &edges, 512, 8192);
        assert_eq!(plan.rows, [Some(0), None, None, Some(1)]);
        assert_eq!(plan.instances.len(), 2);
        assert_eq!((plan.width, plan.height), (512, 8));
        assert_eq!(plan.instances[1].light[3], top_of(&plan, 1));
        assert_eq!(plan.instances[0].light[3], 1.0, "row 0 starts at the top");
        assert_eq!(plan.overflow, 0);
    }

    #[test]
    fn a_spot_light_aimed_away_from_every_edge_gets_no_row() {
        let edges = Edges::new(&[wall_at(2.0)]);
        let aimed = |direction: Vec2, softness: f32| PointLight {
            cone: Some(Cone {
                direction,
                angle: 1.0,
                softness: 0.0,
            }),
            shadow: Some(Shadow {
                strength: 1.0,
                softness,
            }),
            ..lamp(Vec2::ZERO, true)
        };
        let lights = [aimed(Vec2::NEG_X, 0.0), aimed(Vec2::X, 0.0)];
        let plan = ShadowPlan::new(&lights, &edges, 64, 8192);
        assert_eq!(
            plan.rows,
            [None, Some(0)],
            "behind it, the wall is no shadow"
        );
        assert_eq!(plan.instances.len(), 1);

        // Aimed up, the cone's edge is about 28.6° from +y: a wall whose end
        // is at 26.6° is in it, one starting at 33° is not — until a soft
        // edge's taps reach it.
        let up = [aimed(Vec2::Y, 0.0)];
        let edges = Edges::new(&[Occluder {
            points: vec![Vec2::new(1.0, 2.0), Vec2::new(3.0, 2.0)],
            closed: false,
            self_shadow: false,
            layers: crate::lights::ALL_LIGHT_LAYERS,
        }]);
        assert_eq!(ShadowPlan::new(&up, &edges, 64, 8192).rows, [Some(0)]);
        let wall = Edges::new(&[Occluder {
            points: vec![Vec2::new(1.3, 2.0), Vec2::new(3.0, 2.0)],
            closed: false,
            self_shadow: false,
            layers: crate::lights::ALL_LIGHT_LAYERS,
        }]);
        assert_eq!(ShadowPlan::new(&up, &wall, 64, 8192).rows, [None]);
        let soft = [aimed(Vec2::Y, 0.1)];
        assert_eq!(ShadowPlan::new(&soft, &wall, 64, 8192).rows, [Some(0)]);
    }

    #[test]
    fn a_light_gets_no_row_when_no_occluder_shares_its_layers() {
        let wall = Occluder {
            layers: 0b10,
            ..wall_at(2.0)
        };
        let edges = Edges::new(&[wall]);
        let on = |layers| PointLight {
            layers,
            ..lamp(Vec2::ZERO, true)
        };
        let plan = ShadowPlan::new(&[on(0b01), on(0b10)], &edges, 64, 8192);
        assert_eq!(plan.rows, [None, Some(0)]);
    }

    #[test]
    fn rows_round_up_to_a_power_of_two() {
        let edges = Edges::new(&[wall_at(2.0)]);
        let lights = [lamp(Vec2::ZERO, true); 3];
        let plan = ShadowPlan::new(&lights, &edges, 64, 8192);
        assert_eq!(plan.height, 4 * 4, "three lights, four rows of four");
        assert_eq!(plan.instances[0].row_height, 2.0 / 16.0);
    }

    #[test]
    fn lights_past_the_texture_s_height_are_drawn_unshadowed() {
        let edges = Edges::new(&[wall_at(2.0)]);
        let lights = [lamp(Vec2::ZERO, true); 3];
        // Room for two lights' four rows.
        let plan = ShadowPlan::new(&lights, &edges, 64, 8);
        assert_eq!(plan.rows, [Some(0), Some(1), None]);
        assert_eq!(plan.overflow, 1);
        assert_eq!(plan.instances.len(), 2);
        assert_eq!(plan.height, 8);
        assert_eq!(plan.width, 8, "and the width is clamped to the device's");
    }

    #[test]
    fn nothing_to_occlude_is_no_map() {
        let plan = ShadowPlan::new(&[lamp(Vec2::ZERO, true)], &Edges::default(), 64, 8192);
        assert_eq!(plan.rows, [None]);
        assert!(plan.instances.is_empty());
        assert_eq!(plan.height, 0);
        assert_eq!(plan.overflow, 0);
    }
}
