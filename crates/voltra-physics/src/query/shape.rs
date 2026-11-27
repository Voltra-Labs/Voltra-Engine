//! Shape casts: the first surface a box or a circle meets when moved.
//!
//! Exact, not stepped: each pair is solved in closed form, so a fast cast
//! cannot tunnel and a slow one is not approximated.
//!
//! - **Box against box**: the separating axis theorem over time. Each of the
//!   four face axes gives the interval in which the two projections overlap;
//!   the shapes touch first at the latest entry, if it comes before the
//!   earliest exit. Exact for convex polygons, which oriented boxes are.
//! - **Circle against box**: a ray from the circle's centre against the box
//!   grown by the radius, in the box's frame; an entry in a corner's region
//!   is taken against that corner's circle instead.
//! - **Circle against circle**: a ray against the sum of the radii.
//! - **Box against circle**: the circle cast backwards against the box.
//!
//! What Unity's `Physics2D.BoxCast` and Godot's `cast_motion` answer, and the
//! base of [`crate::character`].

use voltra_ecs::Entity;
use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::{Collider2D, OneWay, Transform2D};

use super::{colliders, Poses, QueryFilter};
use crate::narrow::manifold;

mod advance;
mod sweep;

use sweep::sweep;

/// A motion whose part along an axis is less than this share of its length
/// is parallel to it: rounding in a slide along a surface must not read as
/// pressing into it.
const PARALLEL: f32 = 1e-5;

/// An overlap shallower than this is a touch.
const TOUCH: f32 = 1e-5;

/// The first collider a moved shape meets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeHit {
    /// The entity whose collider was met.
    pub entity: Entity,
    /// How much of the motion is free: `0` touches at once, `1` at the end.
    pub fraction: f32,
    /// `fraction` of the motion's length, in world units.
    pub distance: f32,
    /// The surface's normal where they meet, pushing the cast shape away.
    pub normal: Vec2,
    /// How far the shape already overlapped the collider where it started,
    /// along `normal`; zero for a shape that started clear. A cast from
    /// inside hits at fraction zero, as a ray from inside does.
    pub depth: f32,
}

/// The nearest collider `shape`, placed at `at`, meets while moved by
/// `motion`.
///
/// A shape already overlapping something hits it at fraction zero with the
/// overlap's normal and `depth`, so a mover can push out. One merely
/// touching is met only when the motion goes into it: sliding along a floor
/// does not hit the floor. A [`OneWay`] collider is met only from its solid
/// side, and not at all by a shape that starts inside it. A degenerate
/// shape, or a motion that is not finite, finds nothing; a motion of no
/// length finds only overlaps.
pub fn cast(
    world: &World,
    shape: &Collider2D,
    at: &Transform2D,
    motion: Vec2,
    filter: QueryFilter,
) -> Option<ShapeHit> {
    cast_among(world, None, shape, at, motion, filter)
}

/// [`cast`], with `poses` read in place of the world's.
pub(crate) fn cast_among(
    world: &World,
    poses: Option<&Poses>,
    shape: &Collider2D,
    at: &Transform2D,
    motion: Vec2,
    filter: QueryFilter,
) -> Option<ShapeHit> {
    if !motion.is_finite() || shape.is_degenerate(at) {
        return None;
    }
    let (min, max) = swept_bounds(shape, at, motion);
    let mut nearest: Option<ShapeHit> = None;
    for (entity, collider, transform) in colliders(world, poses, filter) {
        let (cmin, cmax) = collider.world_aabb(transform);
        if cmin.x > max.x || cmin.y > max.y || cmax.x < min.x || cmax.y < min.y {
            continue;
        }
        let Some(hit) = hit(shape, at, motion, collider, transform, entity) else {
            continue;
        };
        // A one-way collider is met only on its solid side, and never by a
        // shape already partly through it.
        if let Some(one_way) = world.get::<OneWay>(entity) {
            if hit.depth > TOUCH || !one_way.admits(transform, hit.normal) {
                continue;
            }
        }
        let better = nearest.is_none_or(|best| {
            hit.fraction < best.fraction
                || (hit.fraction == best.fraction && hit.depth > best.depth)
        });
        if better {
            nearest = Some(hit);
        }
    }
    nearest
}

/// The bounds of `shape` over the whole motion.
fn swept_bounds(shape: &Collider2D, at: &Transform2D, motion: Vec2) -> (Vec2, Vec2) {
    let (min, max) = shape.world_aabb(at);
    (min.min(min + motion), max.max(max + motion))
}

fn hit(
    shape: &Collider2D,
    at: &Transform2D,
    motion: Vec2,
    collider: &Collider2D,
    transform: &Transform2D,
    entity: Entity,
) -> Option<ShapeHit> {
    let (enter, normal) = sweep((shape, at), motion, (collider, transform))?;
    if enter > 1.0 {
        return None;
    }
    let length = motion.length();
    if enter > 0.0 {
        return Some(ShapeHit {
            entity,
            fraction: enter,
            distance: enter * length,
            normal,
            depth: 0.0,
        });
    }
    // Overlapping where it starts — or touching and moving in. Overlapping,
    // the overlap's own normal is the way out. Touching, the sweep's face is
    // the surface: the narrow phase's normal for a touch is whichever face it
    // took as reference, which on a corner need not be the one met.
    let overlap = manifold((shape, at), (collider, transform))
        .map(|m| (m.normal, (-m.deepest_separation()).max(0.0)));
    let (normal, depth) = match overlap {
        Some((normal, depth)) if depth > TOUCH => (normal, depth),
        _ if normal != Vec2::ZERO => (normal, 0.0),
        Some((normal, _)) => (normal, 0.0),
        None => return None,
    };
    Some(ShapeHit {
        entity,
        fraction: 0.0,
        distance: 0.0,
        normal,
        depth,
    })
}

#[cfg(test)]
mod tests;
