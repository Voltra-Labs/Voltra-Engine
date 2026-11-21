//! The world's colliders as particles see them: world-space shapes, each
//! with the layers it is on and the box around it, gathered once a step and
//! picked from per emitter.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_render::ParticleCollider;

use crate::{Collider, CollisionLayers, Sensor, Transform};

/// One collider a particle can hit.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Obstacle {
    shape: ParticleCollider,
    layers: u32,
    min: Vec2,
    max: Vec2,
}

/// Every collider particles can hit this step.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Obstacles {
    all: Vec<Obstacle>,
}

impl Obstacles {
    /// Every solid collider in `world`, placed as the physics places it:
    /// by the entity's own [`Transform`]. A [`Sensor`] is left out — it is
    /// detected, never solid — and so is a shape with no area or a
    /// transform that is not a number.
    pub fn gather(world: &World) -> Self {
        let all = world
            .query::<Collider>()
            .filter(|(entity, _)| world.get::<Sensor>(*entity).is_none())
            .filter_map(|(entity, collider)| {
                let transform = world.get::<Transform>(entity)?;
                if collider.is_degenerate(transform) {
                    return None;
                }
                let shape = match collider {
                    Collider::Box { .. } => ParticleCollider::Box {
                        center: transform.translation,
                        rotation: transform.rotation,
                        half_extents: collider.world_half_extents(transform),
                    },
                    Collider::Circle { .. } => ParticleCollider::Circle {
                        center: transform.translation,
                        radius: collider.world_radius(transform),
                    },
                };
                let (min, max) = collider.world_aabb(transform);
                (shape.is_valid() && min.is_finite() && max.is_finite()).then(|| Obstacle {
                    shape,
                    layers: world
                        .get::<CollisionLayers>(entity)
                        .map_or(CollisionLayers::ALL, |layers| layers.layers),
                    min,
                    max,
                })
            })
            .collect();
        Self { all }
    }

    pub fn is_empty(&self) -> bool {
        self.all.is_empty()
    }

    /// The shapes on a layer in `mask` whose box touches `min..max`, in the
    /// world, into `into` (cleared first).
    pub fn near(&self, mask: u32, min: Vec2, max: Vec2, into: &mut Vec<ParticleCollider>) {
        into.clear();
        into.extend(
            self.all
                .iter()
                .filter(|o| {
                    o.layers & mask != 0
                        && o.min.x <= max.x
                        && o.max.x >= min.x
                        && o.min.y <= max.y
                        && o.max.y >= min.y
                })
                .map(|o| o.shape),
        );
    }
}
