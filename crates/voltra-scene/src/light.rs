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
use voltra_render::glam::{Mat3, Vec2};
use voltra_render::{Cone, Lighting, PointLight, Shadow};

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
    /// Makes it a spot light, shining along the entity's +x axis (turn the
    /// entity to aim it). `None` — the default, and not written — shines
    /// every way.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cone: Option<LightCone>,
}

/// A spot light's cone, Unity URP's spot `Light2D`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct LightCone {
    /// The whole angle across the cone, in radians; a full turn or more
    /// shines every way.
    pub angle: f32,
    /// How much of the half-angle, from the edge inward, fades: `0` a hard
    /// edge, `1` from the middle out.
    pub softness: f32,
}

impl Default for LightCone {
    /// Sixty degrees across, its outer quarter fading.
    fn default() -> Self {
        Self {
            angle: std::f32::consts::FRAC_PI_3,
            softness: 0.25,
        }
    }
}

impl PointLight2D {
    /// The light as the renderer takes it, carried by `matrix` (the
    /// entity's composed transform): placed at its origin, its radius
    /// scaled by the larger axis, a spot light's cone along its +x as the
    /// world sees it — turned with it and its parents, flipped by a
    /// mirroring scale.
    pub fn placed(&self, matrix: Mat3) -> PointLight {
        let scale = matrix
            .x_axis
            .truncate()
            .length()
            .max(matrix.y_axis.truncate().length());
        PointLight {
            position: matrix.transform_point2(Vec2::ZERO),
            color: self.color,
            intensity: self.intensity,
            radius: self.radius * scale,
            falloff: self.falloff,
            shadow: self.shadows.map(|shadows| Shadow {
                strength: shadows.strength,
                softness: shadows.softness,
            }),
            cone: self.cone.map(|cone| Cone {
                direction: matrix.transform_vector2(Vec2::X),
                angle: cone.angle,
                softness: cone.softness,
            }),
        }
    }
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
            cone: None,
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
        .map(|(entity, light)| light.placed(transforms.matrix(entity)))
        .filter(|light| {
            let (position, radius) = (light.position, light.radius);
            position.x + radius >= view_min.x
                && position.x - radius <= view_max.x
                && position.y + radius >= view_min.y
                && position.y - radius <= view_max.y
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
mod tests;
