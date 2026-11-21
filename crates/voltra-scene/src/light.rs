//! 2D lights: a point light component, and the frame's lighting gathered
//! from the world.
//!
//! A frame is lit when the camera it is seen through has an
//! [`ambient`](crate::Camera::ambient) colour — Godot's `CanvasModulate`,
//! Unity's global `Light2D` — and then every [`PointLight2D`] in view adds
//! to it. Without one the frame is unlit, as drawn, and lights cost
//! nothing: a scene that never asked for lighting does not go dark the day
//! a light is added to it by accident.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_render::{Lighting, PointLight};

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
}

impl Default for PointLight2D {
    /// A warm lamp three units across.
    fn default() -> Self {
        Self {
            color: [1.0, 0.85, 0.6],
            intensity: 1.0,
            radius: 3.0,
            falloff: 2.0,
        }
    }
}

/// The lighting of the frame seen through the active camera, with the
/// lights whose reach touches `view_min..view_max` — unlit when that
/// camera has no ambient colour, or there is no active camera.
pub fn gather(world: &World, view_min: Vec2, view_max: Vec2) -> Lighting {
    let ambient = camera::active(world)
        .and_then(|entity| world.get::<Camera>(entity))
        .and_then(|camera| camera.ambient);
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
            })
        })
        .collect();
    Lighting { ambient, lights }
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
}
