//! What lights a 3D camera's view: the world's directional, point and spot
//! lights and the environment, gathered for the render side — and, per
//! instance, which of them matter most to it ([`pick`]).
//!
//! **Forward with a light list per object** (ARCHITECTURE "Lights, shadows
//! and the environment"): the view's lights are gathered once — the
//! directional ones (up to four), the point and spot lights whose reach
//! the view's frustum touches (up to 128) — and each drawn instance is
//! given the eight point and spot lights estimated to light it most.
//! Over a cap, the least important are dropped and the frame says so once
//! with the count: never silently.

mod lights;
mod pick;
#[cfg(test)]
mod tests;

pub use pick::{pick, LightReach3D, Picked3D};

use voltra_ecs::{Entity, World};
use voltra_render::render3d::{Lighting3D, View3D};

use crate::environment3d::{Background3D, Environment3D};
use crate::Camera3D;

/// A camera's light, what was left out of it, and what choosing each
/// instance's lights reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Gathered3D {
    /// What the view is lit by: the directional lights, the point and spot
    /// lights (indexed by each instance's list), the ambient, the sky and
    /// the fog.
    pub lighting: Lighting3D,
    /// The entity of each of `lighting.directional`, in order.
    pub directional: Vec<Entity>,
    /// The layers of each of `lighting.directional`, in order.
    pub directional_layers: Vec<u32>,
    /// What [`pick`] weighs each of `lighting.punctual` by, in order.
    pub punctual: Vec<LightReach3D>,
    /// Directional lights left out past [`MAX_DIRECTIONAL`](voltra_render::render3d::MAX_DIRECTIONAL),
    /// the dimmest.
    pub dropped_directional: usize,
    /// Point and spot lights in view left out past
    /// [`MAX_PUNCTUAL`](voltra_render::render3d::MAX_PUNCTUAL), the least
    /// important to the view.
    pub dropped_punctual: usize,
    /// Lights with a number they cannot light by, and why: each warned
    /// about once.
    pub broken: Vec<(Entity, &'static str)>,
    /// The environment used, if any: the camera's own, else the world's.
    pub environment: Option<Entity>,
    /// World environments beyond the one used.
    pub unused_environments: usize,
    /// The environment's background: what the frame clears to or draws
    /// behind.
    pub background: Background3D,
}

/// The light `camera` sees `world` by through `view`: every directional
/// light shining down its entity's −Z (up to four: past that the
/// brightest, in entity order); every point and spot light whose reach the
/// view touches, at its entity's position, a spot shining down its −Z
/// (up to 128: past that the most important to the eye, in entity order);
/// and the camera's own [`Environment3D`] — else the world's, an
/// `Environment3D` on an entity with no camera, the lowest index — else
/// the default's: its ambient times its energy, its gradient sky, its fog.
///
/// A light whose numbers cannot be drawn, or whose entity's transform has
/// no direction, is left out (and listed in `broken`); one that gives no
/// light — energy 0, range 0, black — quietly.
pub fn gather(world: &World, camera: Entity, view: &View3D) -> Gathered3D {
    let lights = lights::gather(world, view);
    let own = world.get::<Environment3D>(camera).map(|env| (camera, *env));
    let mut shared: Vec<(Entity, Environment3D)> = world
        .query::<Environment3D>()
        .filter(|(e, _)| world.get::<Camera3D>(*e).is_none())
        .map(|(e, env)| (e, *env))
        .collect();
    shared.sort_by_key(|(e, _)| e.index());
    let environment = own.or_else(|| shared.first().copied());
    let env = environment.map_or_else(Environment3D::default, |(_, e)| e);
    Gathered3D {
        lighting: Lighting3D {
            directional: lights.directional,
            punctual: lights.punctual,
            ambient: env.ambient_light(),
            sky: env.background.sky(),
            fog: env.fog.map(|fog| fog.fog()),
        },
        directional: lights.directional_entities,
        directional_layers: lights.directional_layers,
        punctual: lights.reach,
        dropped_directional: lights.dropped_directional,
        dropped_punctual: lights.dropped_punctual,
        broken: lights.broken,
        environment: environment.map(|(e, _)| e),
        unused_environments: if own.is_some() {
            0
        } else {
            shared.len().saturating_sub(1)
        },
        background: env.background,
    }
}
