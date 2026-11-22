//! Rays: the nearest surface along a line.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, Transform};

use super::{colliders, normalized, rotated, to_local, QueryFilter};

/// Where a ray met a collider.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    pub entity: Entity,
    /// Where the ray met the surface, in world space.
    pub point: Vec2,
    /// The surface normal there, pointing back along the ray. The reverse of
    /// the ray's own direction when it started inside the shape.
    pub normal: Vec2,
    /// How far along the ray the hit is, in world units.
    pub distance: f32,
}

/// The nearest collider a ray of `max` length meets, if any.
///
/// `direction` need not be normalised. A direction of no length, or a
/// non-positive `max`, finds nothing rather than dividing by it.
///
/// A ray that starts *inside* a shape hits it at distance zero, with the ray
/// reversed as the normal. Reporting nothing there is the classic surprise —
/// a ground check begun a hair inside the floor would say the character is in
/// the air.
pub fn ray(
    world: &World,
    origin: Vec2,
    direction: Vec2,
    max: f32,
    filter: QueryFilter,
) -> Option<RayHit> {
    let direction = normalized(direction)?;
    // NaN included, which is why this is not `max <= 0.0` alone: a length
    // nothing compares to is not a length.
    if max.is_nan() || max <= 0.0 {
        return None;
    }

    let mut nearest: Option<RayHit> = None;
    for (entity, collider, transform) in colliders(world, filter) {
        let Some(hit) = cast(collider, transform, origin, direction, max) else {
            continue;
        };
        let hit = RayHit {
            entity,
            point: hit.0,
            normal: hit.1,
            distance: hit.2,
        };
        if nearest.is_none_or(|best| hit.distance < best.distance) {
            nearest = Some(hit);
        }
    }
    nearest
}

/// `(point, normal, distance)` where a unit ray meets the shape within `max`.
fn cast(
    collider: &Collider,
    transform: &Transform,
    origin: Vec2,
    direction: Vec2,
    max: f32,
) -> Option<(Vec2, Vec2, f32)> {
    let distance = match collider {
        Collider::Box { .. } => {
            let half = collider.world_half_extents(transform);
            slab(
                to_local(transform, origin),
                rotated(-transform.rotation, direction),
                half,
            )?
        }
        Collider::Circle { .. } => {
            let radius = collider.world_radius(transform);
            sphere(origin - transform.translation, direction, radius)?
        }
    };
    if distance > max {
        return None;
    }

    let point = origin + direction * distance;
    // Inside the shape: there is no surface between the origin and the hit, so
    // the only honest normal is the one facing whoever asked.
    let normal = if distance <= 0.0 {
        -direction
    } else {
        match collider {
            Collider::Box { .. } => face_normal(collider, transform, point),
            Collider::Circle { .. } => (point - transform.translation)
                .try_normalize()
                .unwrap_or(-direction),
        }
    };
    Some((point, normal, distance))
}

/// How far a unit ray travels to enter a box of `half` extents, in the box's
/// own frame. Zero when it starts inside.
fn slab(origin: Vec2, direction: Vec2, half: Vec2) -> Option<f32> {
    let mut enter = f32::NEG_INFINITY;
    let mut exit = f32::INFINITY;

    for axis in 0..2 {
        let (o, d, h) = (origin[axis], direction[axis], half[axis]);
        if d.abs() < f32::EPSILON {
            // Parallel to this pair of faces: either it is between them for
            // the whole ray, or it never is.
            if o.abs() > h {
                return None;
            }
            continue;
        }
        let (near, far) = ((-h - o) / d, (h - o) / d);
        enter = enter.max(near.min(far));
        exit = exit.min(near.max(far));
    }

    if enter > exit || exit < 0.0 {
        return None;
    }
    Some(enter.max(0.0))
}

/// How far a unit ray travels to reach a circle of `radius` centred on the
/// origin of `to_centre`. Zero when it starts inside.
fn sphere(to_centre: Vec2, direction: Vec2, radius: f32) -> Option<f32> {
    let along = to_centre.dot(direction);
    let outside = to_centre.length_squared() - radius * radius;
    if outside > 0.0 && along > 0.0 {
        // Outside and pointing away.
        return None;
    }
    let discriminant = along * along - outside;
    if discriminant < 0.0 {
        return None;
    }
    Some((-along - discriminant.sqrt()).max(0.0))
}

/// The outward normal of the box face `point` sits on.
fn face_normal(collider: &Collider, transform: &Transform, point: Vec2) -> Vec2 {
    let local = to_local(transform, point);
    let half = collider.world_half_extents(transform);
    // Whichever axis the point is nearest the edge of, in units of that axis'
    // extent: on a corner both are one, and either face is a correct answer.
    let local = if (local.x.abs() / half.x) >= (local.y.abs() / half.y) {
        Vec2::new(local.x.signum(), 0.0)
    } else {
        Vec2::new(0.0, local.y.signum())
    };
    rotated(transform.rotation, local)
}
