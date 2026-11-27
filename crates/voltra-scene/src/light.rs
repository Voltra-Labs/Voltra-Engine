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

use voltra_assets::{AssetPath, Handle, Textures};
use voltra_ecs::World;
use voltra_render::glam::{Mat3, Vec2};
use voltra_render::{
    Cone, Lighting, PointLight, Shadow, Texture, ALL_LIGHT_LAYERS, DEFAULT_LIGHT_LAYERS,
};

use crate::camera::{self, Camera};
use crate::hierarchy::WorldTransforms;

mod cookies;

use cookies::Cookies;

/// Light shining from the entity's position, fading to nothing at its
/// radius. Scaled with the entity: the radius is in its space.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// How far above the scene it sits, in its entity's units (scaled with
    /// it, as the radius is) — Godot's `Light2D` height. With one, what it
    /// lights is shaded by the angle it reaches it at: through a sprite's
    /// [`normal_map`](crate::Sprite::normal_map), or facing the screen where
    /// there is none, so mapped and plain sprites agree. Lower rakes across
    /// the bumps and dims what is far from it; higher lights from the front.
    /// `None` — the default, and not written — ignores normals.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f32>,
    /// The light layers it lights — Godot's item cull mask: a sprite or
    /// tilemap is lit by it only if its
    /// [`light_layers`](crate::Sprite::light_layers) share a bit with these.
    /// Every bit — the default, and not written — lights everything.
    #[serde(
        default = "all_light_layers",
        skip_serializing_if = "is_all_light_layers"
    )]
    pub layers: u32,
    /// An image laid over the light's reach that tints and shapes it — a
    /// window's panes thrown on the floor — by path, read as colour: Godot's
    /// `PointLight2D` texture. It spans the square the radius reaches along
    /// the entity's axes, so it turns and mirrors with it. `None` — the
    /// default, and not written — shines plain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cookie: Option<AssetPath>,
    /// The handle `cookie` resolves to. Never saved.
    #[serde(skip)]
    pub cookie_handle: Option<Handle<Texture>>,
}

/// What a light's layers are when nothing says otherwise: all of them.
pub(crate) fn all_light_layers() -> u32 {
    ALL_LIGHT_LAYERS
}

pub(crate) fn is_all_light_layers(layers: &u32) -> bool {
    *layers == ALL_LIGHT_LAYERS
}

/// What a surface's light layers are when nothing says otherwise: the
/// first, as Godot's `light_mask` starts.
pub(crate) fn default_light_layers() -> u32 {
    DEFAULT_LIGHT_LAYERS
}

pub(crate) fn is_default_light_layers(layers: &u32) -> bool {
    *layers == DEFAULT_LIGHT_LAYERS
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
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "PointLight2D";

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
            height: self.height.map(|height| height * scale),
            layers: self.layers,
            // Which image is the frame's to number: `gather` wears it.
            cookie: None,
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
            height: None,
            layers: ALL_LIGHT_LAYERS,
            cookie: None,
            cookie_handle: None,
        }
    }
}

/// The lighting of the frame seen through the active camera, with the
/// lights whose reach — a spot light's cone — touches `view_min..view_max`
/// and, when one of them
/// casts shadows, every caster's outline — unlit when that camera has no
/// ambient colour, or there is no active camera. Lights wear their cookies
/// from `textures`; without it they shine plain.
pub fn gather(
    world: &World,
    view_min: Vec2,
    view_max: Vec2,
    sprites: Option<crate::shadow_caster::SpriteShapes<'_>>,
    textures: Option<&Textures>,
) -> Lighting {
    let Some(camera) = camera::active(world).and_then(|entity| world.get::<Camera>(entity)) else {
        return Lighting::default();
    };
    gather_through(world, camera, view_min, view_max, sprites, textures)
}

/// [`gather`] for the frame seen through `camera` — one of several a frame
/// draws, each with its own ambient colour and shadow resolution.
pub fn gather_through(
    world: &World,
    camera: &Camera,
    view_min: Vec2,
    view_max: Vec2,
    sprites: Option<crate::shadow_caster::SpriteShapes<'_>>,
    textures: Option<&Textures>,
) -> Lighting {
    let ambient = camera.ambient;
    if ambient.is_none() {
        return Lighting::default();
    }
    let transforms = WorldTransforms::from_world(world);
    let mut cookies = Cookies::new(textures);
    let lights = world
        .query::<PointLight2D>()
        .map(|(entity, light)| {
            let matrix = transforms.matrix(entity);
            (light, matrix, light.placed(matrix))
        })
        // By the box it reaches: a spot light aimed away from the view is
        // left out even when its disc would touch it. One that gives
        // nothing reaches nowhere and is left out too.
        .filter(|(_, _, placed)| {
            placed.reach().is_some_and(|reach| {
                let (min, max) = reach.bounds();
                max.x >= view_min.x
                    && min.x <= view_max.x
                    && max.y >= view_min.y
                    && min.y <= view_max.y
            })
        })
        // Worn after culling, so the frame binds only what it draws.
        .map(|(light, matrix, placed)| PointLight {
            cookie: cookies.wear(light, matrix),
            ..placed
        })
        .collect::<Vec<_>>();
    // An occluder out of view can still throw a shadow into it, so casters
    // are not culled by the view; the renderer culls them per light.
    let occluders = if lights.iter().any(|light| light.shadow.is_some()) {
        crate::shadow_caster::occluders(world, &transforms, sprites)
    } else {
        Vec::new()
    };
    Lighting {
        ambient,
        lights,
        globals: crate::global_light::gather(world),
        cookies: cookies.into_bind_groups(),
        occluders,
        shadow_resolution: camera.shadow_resolution,
    }
}

#[cfg(test)]
mod tests;
