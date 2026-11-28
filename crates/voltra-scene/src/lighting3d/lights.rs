//! The world's 3D lights as one view takes them: placed, checked, culled
//! against its frustum and kept within the caps.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec3;
use voltra_render::render3d::{
    DirectionalLight, PunctualLight, SpotCone, View3D, MAX_DIRECTIONAL, MAX_PUNCTUAL,
};

use super::pick::LightReach3D;
use crate::light3d::intensity;
use crate::{DirectionalLight3D, PointLight3D, SpotLight3D};

/// What [`gather`] finds.
#[derive(Debug, Default)]
pub(super) struct Lights {
    pub directional: Vec<DirectionalLight>,
    pub directional_entities: Vec<Entity>,
    pub directional_layers: Vec<u32>,
    pub punctual: Vec<PunctualLight>,
    pub reach: Vec<LightReach3D>,
    pub dropped_directional: usize,
    pub dropped_punctual: usize,
    pub broken: Vec<(Entity, &'static str)>,
}

/// The way `entity`'s −Z points in the world, or `None` where its
/// transform has none.
fn down_minus_z(world: &World, entity: Entity) -> Option<(Vec3, Vec3)> {
    let matrix = crate::hierarchy::world_matrix_3d(world, entity);
    let direction = matrix.transform_vector3(Vec3::NEG_Z).normalize_or_zero();
    let position = Vec3::from(matrix.translation);
    (direction != Vec3::ZERO && direction.is_finite() && position.is_finite())
        .then_some((position, direction))
}

/// Every light of `world` that lights `view`, in entity order within each
/// kind, capped as [`gather`](super::gather) says.
pub(super) fn gather(world: &World, view: &View3D) -> Lights {
    let mut out = Lights::default();

    // Directional: every one, the brightest four if more, in entity order.
    let mut suns: Vec<(Entity, DirectionalLight, u32)> = Vec::new();
    for (entity, light) in world.query::<DirectionalLight3D>() {
        if let Some(problem) = light.problem() {
            out.broken.push((entity, problem));
            continue;
        }
        let Some((_, direction)) = down_minus_z(world, entity) else {
            out.broken
                .push((entity, "its transform has no direction; it lights nothing"));
            continue;
        };
        suns.push((
            entity,
            DirectionalLight {
                direction,
                color: light.color,
                energy: light.energy,
            },
            light.layers,
        ));
    }
    suns.sort_by_key(|(e, _, _)| e.index());
    if suns.len() > MAX_DIRECTIONAL {
        out.dropped_directional = suns.len() - MAX_DIRECTIONAL;
        let brightness = |l: &DirectionalLight| intensity(&l.color, l.energy);
        suns.sort_by(|a, b| {
            brightness(&b.1)
                .total_cmp(&brightness(&a.1))
                .then(a.0.index().cmp(&b.0.index()))
        });
        suns.truncate(MAX_DIRECTIONAL);
        suns.sort_by_key(|(e, _, _)| e.index());
    }
    for (entity, light, layers) in suns {
        out.directional.push(light);
        out.directional_entities.push(entity);
        out.directional_layers.push(layers);
    }

    // Point and spot: those whose reach the view touches.
    let frustum = view.frustum();
    let points = world.query::<PointLight3D>().map(|(entity, l)| Lamp {
        entity,
        problem: l.problem(),
        color: l.color,
        energy: l.energy,
        range: l.range,
        layers: l.layers,
        cone: None,
    });
    let spots = world.query::<SpotLight3D>().map(|(entity, l)| Lamp {
        entity,
        problem: l.problem(),
        color: l.color,
        energy: l.energy,
        range: l.range,
        layers: l.layers,
        cone: Some((l.inner_angle, l.outer_angle)),
    });
    let mut lamps: Vec<(PunctualLight, LightReach3D)> = Vec::new();
    for lamp in points.chain(spots) {
        if let Some(problem) = lamp.problem {
            out.broken.push((lamp.entity, problem));
            continue;
        }
        let strength = intensity(&lamp.color, lamp.energy);
        if strength <= 0.0 || lamp.range <= 0.0 || lamp.layers == 0 {
            continue;
        }
        let Some((position, direction)) = down_minus_z(world, lamp.entity) else {
            out.broken.push((
                lamp.entity,
                "its transform has no direction; it lights nothing",
            ));
            continue;
        };
        if !frustum.intersects_sphere(position, lamp.range) {
            continue;
        }
        let light = PunctualLight {
            position,
            color: lamp.color,
            energy: lamp.energy,
            range: lamp.range,
            spot: lamp.cone.map(|(inner, outer)| SpotCone {
                direction,
                inner: inner.to_radians(),
                outer: outer.to_radians(),
            }),
        };
        let reach = LightReach3D::new(lamp.entity.index(), &light, strength, lamp.layers);
        lamps.push((light, reach));
    }
    if lamps.len() > MAX_PUNCTUAL {
        out.dropped_punctual = lamps.len() - MAX_PUNCTUAL;
        // The most important to the eye: what reaches the nearest edge of
        // its sphere.
        let eye = view.eye;
        let weight = |(light, reach): &(PunctualLight, LightReach3D)| {
            let gap = ((light.position - eye).length() - light.range).max(0.0);
            reach.intensity / gap.mul_add(gap, 1.0)
        };
        lamps.sort_by(|a, b| {
            weight(b)
                .total_cmp(&weight(a))
                .then(a.1.entity.cmp(&b.1.entity))
        });
        lamps.truncate(MAX_PUNCTUAL);
    }
    // In entity order: the array's order does not follow the camera.
    lamps.sort_by_key(|(_, reach)| reach.entity);
    for (light, reach) in lamps {
        out.punctual.push(light);
        out.reach.push(reach);
    }
    out
}

/// A point or spot light as found, before it is placed.
struct Lamp {
    entity: Entity,
    problem: Option<&'static str>,
    color: [f32; 3],
    energy: f32,
    range: f32,
    layers: u32,
    /// A spot's inner and outer angles, in degrees.
    cone: Option<(f32, f32)>,
}
