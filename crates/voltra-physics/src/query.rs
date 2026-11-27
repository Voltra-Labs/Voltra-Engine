//! Asking the world a question without touching it.
//!
//! A step reports what collided; a query answers what *would*. Every engine
//! has the pair — Unity's `Physics2D.Raycast`, Godot's
//! `PhysicsDirectSpaceState2D.intersect_ray`, Box2D's `b2World::RayCast`,
//! Rapier's `QueryPipeline` — because gameplay asks constantly: what is under
//! the character's feet, what does this shot hit, what is inside the blast.
//!
//! These take `&World` and nothing else, so they work from either of a game's
//! ticks, from the editor, and from a test with no [`PhysicsWorld`] at all.
//!
//! [`PhysicsWorld`]: crate::PhysicsWorld

mod filter;
mod poses;
mod ray;
mod shape;

pub use filter::QueryFilter;
pub(crate) use poses::Poses;
pub(crate) use ray::ray_among;
pub use ray::{ray, RayHit};
pub(crate) use shape::cast_among;
pub use shape::{cast, ShapeHit};

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{Collider2D, Transform2D};

use crate::geometry::Hull;
use crate::narrow::manifold;

/// A collider covering `at`, if any.
///
/// Where several cover it, which one is reported is the order the world stores
/// them in. Use [`overlap_aabb`] when every answer matters — a point query is
/// for "is there anything here", which is how Godot's and Unity's point checks
/// are used too.
pub fn point(world: &World, at: Vec2, filter: QueryFilter) -> Option<Entity> {
    point_among(world, None, at, filter)
}

/// [`point`], with `poses` read in place of the world's.
pub(crate) fn point_among(
    world: &World,
    poses: Option<&Poses>,
    at: Vec2,
    filter: QueryFilter,
) -> Option<Entity> {
    colliders(world, poses, filter)
        .find(|(_, collider, transform)| contains(collider, transform, at))
        .map(|(entity, _, _)| entity)
}

/// Every collider overlapping the box `(min, max)`.
///
/// The *shapes*, not their bounds: the box is collided against each collider
/// exactly as a step would, so a rotated crate whose bounds cover the corner
/// of the query but whose body does not is not reported. Merely touching does
/// not count, for the same reason it does not in a step.
///
/// An inverted or empty box finds nothing.
pub fn overlap_aabb(world: &World, min: Vec2, max: Vec2, filter: QueryFilter) -> Vec<Entity> {
    overlap_aabb_among(world, None, min, max, filter)
}

/// [`overlap_aabb`], with `poses` read in place of the world's.
pub(crate) fn overlap_aabb_among(
    world: &World,
    poses: Option<&Poses>,
    min: Vec2,
    max: Vec2,
    filter: QueryFilter,
) -> Vec<Entity> {
    let half = (max - min) * 0.5;
    if half.x <= 0.0 || half.y <= 0.0 {
        return Vec::new();
    }
    let query = Collider2D::Box { half_extents: half };
    let at = Transform2D::from_translation(min + half);

    colliders(world, poses, filter)
        .filter(|(_, collider, transform)| manifold((&query, &at), (collider, transform)).is_some())
        .map(|(entity, _, _)| entity)
        .collect()
}

/// Every collider this query may find, with where it sits: as the world
/// has it, or as `poses` says for the entities it names — hidden for one
/// it holds no pose of.
pub(super) fn colliders<'a>(
    world: &'a World,
    poses: Option<&'a Poses>,
    filter: QueryFilter,
) -> impl Iterator<Item = (Entity, &'a Collider2D, &'a Transform2D)> {
    world.query::<Collider2D>().filter_map(move |(entity, c)| {
        let (c, transform) = match poses.and_then(|poses| poses.get(&entity)) {
            Some(Some((collider, transform))) => (collider, transform),
            Some(None) => return None,
            None => (c, world.get::<Transform2D>(entity)?),
        };
        // A shape with no area is not a surface: it would report a hit at a
        // point nothing is drawn at, and `is_degenerate` is what the narrow
        // phase rejects it with too.
        if c.is_degenerate(transform) || !filter.allows(world, entity) {
            return None;
        }
        Some((entity, c, transform))
    })
}

/// Whether `at` is inside the shape.
fn contains(collider: &Collider2D, transform: &Transform2D, at: Vec2) -> bool {
    match collider {
        Collider2D::Box { .. } => {
            let local = to_local(transform, at);
            let half = collider.world_half_extents(transform);
            local.x.abs() <= half.x && local.y.abs() <= half.y
        }
        Collider2D::Circle { .. } => {
            let radius = collider.world_radius(transform);
            at.distance_squared(transform.translation) <= radius * radius
        }
        Collider2D::Capsule { .. } | Collider2D::Polygon { .. } => {
            let hull = Hull::of(collider, transform);
            let (nearest, inside) = hull.nearest(at);
            inside || at.distance_squared(nearest) <= hull.radius * hull.radius
        }
    }
}

/// `point` in the shape's own frame: rotation undone, centre at the origin.
pub(super) fn to_local(transform: &Transform2D, point: Vec2) -> Vec2 {
    rotated(-transform.rotation, point - transform.translation)
}

pub(super) fn rotated(radians: f32, vector: Vec2) -> Vec2 {
    Vec2::from_angle(radians).rotate(vector)
}

/// The unit vector along `direction`, or nothing if it has no length.
pub(super) fn normalized(direction: Vec2) -> Option<Vec2> {
    direction.try_normalize()
}

#[cfg(test)]
mod tests;
