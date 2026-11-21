//! 2D lights: a point light component, and the frame's lighting gathered
//! from the world.
//!
//! A frame is lit when the camera it is seen through has an
//! [`ambient`](crate::Camera::ambient) colour — Godot's `CanvasModulate`,
//! Unity's global `Light2D` — and then every [`PointLight2D`] in view adds
//! to it. Without one the frame is unlit, as drawn, and lights cost
//! nothing: a scene that never asked for lighting does not go dark the day
//! a light is added to it by accident.
//!
//! A light with [`shadows`](PointLight2D::shadows) is blocked by every
//! [`ShadowCaster2D`](crate::ShadowCaster2D); casters are gathered only when
//! some light in view asks for them.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_render::{Lighting, PointLight, Shadow};

use crate::camera::{self, Camera};
use crate::hierarchy::WorldTransforms;

/// Light shining from the entity's position, fading to nothing at its
/// radius. Scaled with the entity: the radius is in its space.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PointLight2D {
    /// Linear colour.
    pub color: [f32; 3],
    /// What the colour is multiplied by; past one pushes what it lights
    /// past white, into the bloom.
    pub intensity: f32,
    /// Units from the centre to where it gives nothing.
    pub radius: f32,
    /// How it fades: `1` a straight ramp, `2` soft, higher kept close in.
    pub falloff: f32,
    /// Blocked by [`ShadowCaster2D`](crate::ShadowCaster2D)s. `None` — the
    /// default, and not written — shines through everything.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadows: Option<LightShadows>,
}

/// How a light's shadows look.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct LightShadows {
    /// How much of the light a shadow takes away: `1` all, `0` none.
    pub strength: f32,
    /// How far either side of a shadow's edge it fades, as an angle around
    /// the light in radians; `0` is a hard edge. At most
    /// [`MAX_SHADOW_SOFTNESS`](voltra_render::lights::MAX_SHADOW_SOFTNESS).
    pub softness: f32,
}

impl Default for LightShadows {
    /// Full, with an edge softened by about a degree.
    fn default() -> Self {
        Self {
            strength: 1.0,
            softness: 0.02,
        }
    }
}

impl Default for PointLight2D {
    /// A warm lamp three units across.
    fn default() -> Self {
        Self {
            color: [1.0, 0.85, 0.6],
            intensity: 1.0,
            radius: 3.0,
            falloff: 2.0,
            shadows: None,
        }
    }
}

/// The lighting of the frame seen through the active camera, with the
/// lights whose reach touches `view_min..view_max` and, when one of them
/// casts shadows, every caster's outline — unlit when that camera has no
/// ambient colour, or there is no active camera.
pub fn gather(world: &World, view_min: Vec2, view_max: Vec2) -> Lighting {
    let Some(camera) = camera::active(world).and_then(|entity| world.get::<Camera>(entity)) else {
        return Lighting::default();
    };
    let ambient = camera.ambient;
    if ambient.is_none() {
        return Lighting::default();
    }
    let transforms = WorldTransforms::from_world(world);
    let lights = world
        .query::<PointLight2D>()
        .filter_map(|(entity, light)| {
            let matrix = transforms.matrix(entity);
            let scale = matrix
                .x_axis
                .truncate()
                .length()
                .max(matrix.y_axis.truncate().length());
            let position = matrix.transform_point2(Vec2::ZERO);
            let radius = light.radius * scale;
            let seen = position.x + radius >= view_min.x
                && position.x - radius <= view_max.x
                && position.y + radius >= view_min.y
                && position.y - radius <= view_max.y;
            seen.then_some(PointLight {
                position,
                color: light.color,
                intensity: light.intensity,
                radius,
                falloff: light.falloff,
                shadow: light.shadows.map(|shadows| Shadow {
                    strength: shadows.strength,
                    softness: shadows.softness,
                }),
            })
        })
        .collect::<Vec<_>>();
    // An occluder out of view can still throw a shadow into it, so casters
    // are not culled by the view; the renderer culls them per light.
    let occluders = if lights.iter().any(|light| light.shadow.is_some()) {
        crate::shadow_caster::occluders(world, &transforms)
    } else {
        Vec::new()
    };
    Lighting {
        ambient,
        lights,
        occluders,
        shadow_resolution: camera.shadow_resolution,
    }
}

#[cfg(test)]
mod tests {
    use voltra_render::glam::Vec2;

    use super::*;
    use crate::{hierarchy, Transform};

    fn lit_camera(world: &mut World, ambient: Option<[f32; 3]>) {
        let camera = world.spawn();
        world.insert(camera, Transform::default());
        world.insert(
            camera,
            Camera {
                ambient,
                ..Camera::default()
            },
        );
    }

    fn lamp(world: &mut World, at: Vec2) -> voltra_ecs::Entity {
        let lamp = world.spawn();
        world.insert(lamp, Transform::from_translation(at));
        world.insert(lamp, PointLight2D::default());
        lamp
    }

    const VIEW: (Vec2, Vec2) = (Vec2::new(-10.0, -10.0), Vec2::new(10.0, 10.0));

    #[test]
    fn without_an_ambient_the_frame_is_unlit_whatever_lights_there_are() {
        let mut world = World::new();
        lit_camera(&mut world, None);
        lamp(&mut world, Vec2::ZERO);
        assert_eq!(gather(&world, VIEW.0, VIEW.1), Lighting::default());
    }

    #[test]
    fn lights_in_reach_of_the_view_are_gathered_and_the_rest_left_out() {
        let mut world = World::new();
        lit_camera(&mut world, Some([0.1; 3]));
        lamp(&mut world, Vec2::new(1.0, 2.0));
        // Off the view's right edge, but its radius of 3 reaches back in.
        lamp(&mut world, Vec2::new(12.0, 0.0));
        lamp(&mut world, Vec2::new(100.0, 0.0));
        let lighting = gather(&world, VIEW.0, VIEW.1);
        assert_eq!(lighting.ambient, Some([0.1; 3]));
        let at: Vec<Vec2> = lighting.lights.iter().map(|l| l.position).collect();
        assert_eq!(at.len(), 2, "{at:?}");
        assert!(at.contains(&Vec2::new(1.0, 2.0)) && at.contains(&Vec2::new(12.0, 0.0)));
    }

    #[test]
    fn a_light_follows_its_parent_and_scales_with_it() {
        let mut world = World::new();
        lit_camera(&mut world, Some([0.1; 3]));
        let parent = world.spawn();
        world.insert(
            parent,
            Transform::from_translation(Vec2::new(4.0, 0.0)).with_scale(Vec2::splat(2.0)),
        );
        world.insert(parent, crate::SceneId::new());
        let child = lamp(&mut world, Vec2::new(1.0, 0.0));
        hierarchy::set_parent(&mut world, child, parent).expect("no cycle");
        let light = gather(&world, VIEW.0, VIEW.1).lights[0];
        assert!(
            (light.position - Vec2::new(6.0, 0.0)).length() < 1e-5,
            "{light:?}"
        );
        assert!((light.radius - 6.0).abs() < 1e-5);
    }

    #[test]
    fn a_light_round_trips_through_ron_and_fills_what_is_left_out() {
        let light: PointLight2D = ron::from_str("(radius: 5.0)").expect("reads");
        assert_eq!(light.radius, 5.0);
        assert_eq!(light.falloff, 2.0);
        let text = ron::to_string(&light).expect("writes");
        assert_eq!(ron::from_str::<PointLight2D>(&text).expect("reads"), light);
    }

    #[test]
    fn casters_are_gathered_only_for_a_light_that_casts_shadows() {
        let mut world = World::new();
        lit_camera(&mut world, Some([0.1; 3]));
        let lamp = lamp(&mut world, Vec2::ZERO);
        let wall = world.spawn();
        world.insert(wall, Transform::from_translation(Vec2::new(2.0, 0.0)));
        world.insert(
            wall,
            crate::Collider::Box {
                half_extents: Vec2::ONE,
            },
        );
        world.insert(wall, crate::ShadowCaster2D::default());
        assert!(gather(&world, VIEW.0, VIEW.1).occluders.is_empty());

        world
            .get_mut::<PointLight2D>(lamp)
            .expect("spawned with one")
            .shadows = Some(LightShadows::default());
        let lighting = gather(&world, VIEW.0, VIEW.1);
        assert_eq!(lighting.occluders.len(), 1);
        let shadow = lighting.lights[0].shadow.expect("casts");
        assert_eq!(shadow.strength, 1.0);
    }

    #[test]
    fn shadows_are_left_out_of_the_file_when_off() {
        let text = ron::to_string(&PointLight2D::default()).expect("writes");
        assert!(!text.contains("shadows"), "{text}");
        let lit: PointLight2D = ron::from_str("(shadows: Some((strength: 0.5)))").expect("reads");
        assert_eq!(
            lit.shadows,
            Some(LightShadows {
                strength: 0.5,
                ..LightShadows::default()
            })
        );
    }

    #[test]
    fn the_shadow_map_s_resolution_is_the_camera_s_and_left_out_when_default() {
        let mut world = World::new();
        let camera = world.spawn();
        world.insert(
            camera,
            Camera {
                ambient: Some([0.1; 3]),
                shadow_resolution: 256,
                ..Camera::default()
            },
        );
        assert_eq!(gather(&world, VIEW.0, VIEW.1).shadow_resolution, 256);
        let text = ron::to_string(&Camera::default()).expect("writes");
        assert!(!text.contains("shadow_resolution"), "{text}");
        let read: Camera = ron::from_str(&text).expect("reads");
        assert_eq!(
            read.shadow_resolution,
            voltra_render::lights::DEFAULT_SHADOW_RESOLUTION
        );
    }
}
