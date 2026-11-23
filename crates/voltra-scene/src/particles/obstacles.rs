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
        let mut all = Vec::new();
        for (entity, collider) in world.query::<Collider>() {
            if world.get::<Sensor>(entity).is_some() {
                continue;
            }
            let Some(transform) = world.get::<Transform>(entity) else {
                continue;
            };
            if collider.is_degenerate(transform) {
                continue;
            }
            let (min, max) = collider.world_aabb(transform);
            if !(min.is_finite() && max.is_finite()) {
                continue;
            }
            let layers = world
                .get::<CollisionLayers>(entity)
                .map_or(CollisionLayers::ALL, |layers| layers.layers);
            all.extend(
                shapes(collider, transform)
                    .into_iter()
                    .flatten()
                    .filter(ParticleCollider::is_valid)
                    .map(|shape| Obstacle {
                        shape,
                        layers,
                        min,
                        max,
                    }),
            );
        }
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

/// The particle shapes `collider` is, placed: a box or a circle as itself,
/// a capsule exactly as the box between its caps and the two caps, and a
/// polygon as the turned box around its points.
///
/// Particles test boxes and circles only, on the CPU and on the GPU alike,
/// and a polygon is the one shape that is not a union of them; its bounding
/// box keeps particles off it at the price of a corner that is too square.
fn shapes(collider: &Collider, transform: &Transform) -> [Option<ParticleCollider>; 3] {
    let circle = |center| ParticleCollider::Circle {
        center,
        radius: collider.world_radius(transform),
    };
    match collider {
        Collider::Box { .. } => [
            Some(ParticleCollider::Box {
                center: transform.translation,
                rotation: transform.rotation,
                half_extents: collider.world_half_extents(transform),
            }),
            None,
            None,
        ],
        Collider::Circle { .. } => [Some(circle(transform.translation)), None, None],
        Collider::Capsule { .. } => {
            let shape = collider.world_shape(transform);
            match *shape.vertices() {
                [low, high] => [
                    Some(ParticleCollider::Box {
                        center: (low + high) * 0.5,
                        rotation: transform.rotation,
                        half_extents: Vec2::new(shape.radius(), (high - low).length() * 0.5),
                    }),
                    Some(circle(low)),
                    Some(circle(high)),
                ],
                _ => [Some(circle(transform.translation)), None, None],
            }
        }
        Collider::Polygon { points, .. } => {
            let scaled = points.points().iter().map(|p| *p * transform.scale);
            let (lo, hi) = scaled.fold(
                (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
                |(lo, hi), p| (lo.min(p), hi.max(p)),
            );
            let grow = Vec2::splat(collider.world_radius(transform));
            let middle = Vec2::from_angle(transform.rotation).rotate((lo + hi) * 0.5);
            [
                Some(ParticleCollider::Box {
                    center: transform.translation + middle,
                    rotation: transform.rotation,
                    half_extents: (hi - lo) * 0.5 + grow,
                }),
                None,
                None,
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConvexPolygon;

    fn gathered(collider: Collider) -> Vec<ParticleCollider> {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform::from_translation(Vec2::new(1.0, 0.0)));
        world.insert(entity, collider);
        let obstacles = Obstacles::gather(&world);
        let mut near = Vec::new();
        obstacles.near(u32::MAX, Vec2::splat(-10.0), Vec2::splat(10.0), &mut near);
        near
    }

    #[test]
    fn a_capsule_is_its_middle_box_and_its_two_caps() {
        let shapes = gathered(Collider::Capsule {
            half_height: 1.0,
            radius: 0.25,
        });

        assert_eq!(shapes.len(), 3, "{shapes:?}");
        assert!(shapes.contains(&ParticleCollider::Circle {
            center: Vec2::new(1.0, 0.75),
            radius: 0.25
        }));
        assert!(shapes.contains(&ParticleCollider::Box {
            center: Vec2::new(1.0, 0.0),
            rotation: 0.0,
            half_extents: Vec2::new(0.25, 0.75),
        }));
    }

    #[test]
    fn a_polygon_is_the_box_around_its_points() {
        let points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(0.0, 1.0),
        ];
        let shapes = gathered(Collider::Polygon {
            points: ConvexPolygon::new(&points).expect("a triangle"),
            radius: 0.0,
        });

        assert_eq!(
            shapes,
            vec![ParticleCollider::Box {
                center: Vec2::new(2.0, 0.5),
                rotation: 0.0,
                half_extents: Vec2::new(1.0, 0.5),
            }]
        );
    }
}
