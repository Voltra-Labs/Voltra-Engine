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
use voltra_scene::{Collider, OneWay, Transform};

use super::{colliders, rotated, to_local, QueryFilter};
use crate::narrow::manifold;

/// A motion whose part along an axis is less than this share of its length
/// is parallel to it: rounding in a slide along a surface must not read as
/// pressing into it.
const PARALLEL: f32 = 1e-5;

/// An overlap shallower than this is a touch.
const TOUCH: f32 = 1e-5;

/// The first collider a moved shape meets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeHit {
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
    shape: &Collider,
    at: &Transform,
    motion: Vec2,
    filter: QueryFilter,
) -> Option<ShapeHit> {
    if !motion.is_finite() || shape.is_degenerate(at) {
        return None;
    }
    let (min, max) = swept_bounds(shape, at, motion);
    let mut nearest: Option<ShapeHit> = None;
    for (entity, collider, transform) in colliders(world, filter) {
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
fn swept_bounds(shape: &Collider, at: &Transform, motion: Vec2) -> (Vec2, Vec2) {
    let (min, max) = shape.world_aabb(at);
    (min.min(min + motion), max.max(max + motion))
}

fn hit(
    shape: &Collider,
    at: &Transform,
    motion: Vec2,
    collider: &Collider,
    transform: &Transform,
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

type Shape<'a> = (&'a Collider, &'a Transform);

/// When, as a fraction of `motion`, `a` first touches `b`, and the normal
/// pushing `a` away there. Negative while they start overlapping, with a
/// normal that may be zero if no face is to blame.
fn sweep(a: Shape<'_>, motion: Vec2, b: Shape<'_>) -> Option<(f32, Vec2)> {
    match (a.0, b.0) {
        (Collider::Box { .. }, Collider::Box { .. }) => box_box(a, motion, b),
        (Collider::Circle { .. }, Collider::Box { .. }) => {
            circle_box(a.1.translation, a.0.world_radius(a.1), motion, b)
        }
        (Collider::Box { .. }, Collider::Circle { .. }) => {
            // The circle moving back against the box; its normal reversed.
            circle_box(b.1.translation, b.0.world_radius(b.1), -motion, a)
                .map(|(t, normal)| (t, -normal))
        }
        (Collider::Circle { .. }, Collider::Circle { .. }) => {
            let radius = a.0.world_radius(a.1) + b.0.world_radius(b.1);
            let from = a.1.translation - b.1.translation;
            let t = circle_entry(from, motion, radius)?;
            let normal = (from + motion * t.max(0.0)).normalize_or_zero();
            Some((t, normal))
        }
    }
}

/// Swept SAT over both boxes' face axes.
fn box_box(a: Shape<'_>, motion: Vec2, b: Shape<'_>) -> Option<(f32, Vec2)> {
    let (corners_a, corners_b) = (a.0.corners(a.1), b.0.corners(b.1));
    let axes = [
        Vec2::from_angle(a.1.rotation),
        Vec2::from_angle(a.1.rotation).perp(),
        Vec2::from_angle(b.1.rotation),
        Vec2::from_angle(b.1.rotation).perp(),
    ];
    let still = motion.length() * PARALLEL;
    let mut enter = f32::NEG_INFINITY;
    let mut exit = f32::INFINITY;
    let mut normal = Vec2::ZERO;
    for axis in axes {
        let (a0, a1) = project(&corners_a, axis);
        let (b0, b1) = project(&corners_b, axis);
        let speed = motion.dot(axis);
        if speed.abs() <= still {
            // Still along this axis: apart for good — touching counts — or
            // overlapping for good.
            if a1 <= b0 + TOUCH || a0 >= b1 - TOUCH {
                return None;
            }
            continue;
        }
        // Leaving a touch along this axis is apart: what overlap is left
        // to cross is no more than rounding.
        let (behind, ahead) = if speed > 0.0 {
            (b1 - a0, b0 - a1)
        } else {
            (a1 - b0, a0 - b1)
        };
        if behind <= TOUCH {
            return None;
        }
        let (first, last, face) = if speed > 0.0 {
            (ahead / speed, behind / speed, -axis)
        } else {
            (-ahead / speed, -behind / speed, axis)
        };
        if first > enter {
            enter = first;
            normal = face;
        }
        exit = exit.min(last);
        if enter >= exit {
            return None;
        }
    }
    // Leaving a touch, or moving apart: no hit.
    if exit <= 0.0 {
        return None;
    }
    Some((enter, normal))
}

/// A circle at `centre` of `radius`, moved by `motion`, against the box `b`.
fn circle_box(centre: Vec2, radius: f32, motion: Vec2, b: Shape<'_>) -> Option<(f32, Vec2)> {
    let half = b.0.world_half_extents(b.1);
    let from = to_local(b.1, centre);
    let along = rotated(-b.1.rotation, motion);
    let grown = half + Vec2::splat(radius);

    let still = motion.length() * PARALLEL;
    let mut enter = f32::NEG_INFINITY;
    let mut exit = f32::INFINITY;
    let mut face = Vec2::ZERO;
    for axis in 0..2 {
        let (o, d, h) = (from[axis], along[axis], grown[axis]);
        if d.abs() <= still {
            if o.abs() >= h - TOUCH {
                return None;
            }
            continue;
        }
        // Leaving a touch on this axis.
        if (h - o * d.signum()) <= TOUCH {
            return None;
        }
        let (near, far) = ((-h - o) / d, (h - o) / d);
        let (first, last) = (near.min(far), near.max(far));
        if first > enter {
            enter = first;
            face = Vec2::ZERO;
            face[axis] = if d > 0.0 { -1.0 } else { 1.0 };
        }
        exit = exit.min(last);
    }
    if enter >= exit || exit <= 0.0 {
        return None;
    }

    // In a corner's region the grown box is rounder than its faces: the
    // corner's own circle decides.
    let at = from + along * enter.max(0.0);
    let (t, normal) = if at.x.abs() > half.x && at.y.abs() > half.y {
        let corner = Vec2::new(half.x.copysign(at.x), half.y.copysign(at.y));
        let t = circle_entry(from - corner, along, radius)?;
        (t, (from + along * t.max(0.0) - corner).normalize_or_zero())
    } else {
        (enter, face)
    };
    Some((t, rotated(b.1.rotation, normal)))
}

/// The first `t` at which `from + motion · t` is `radius` from the origin.
/// Negative while it starts inside; `None` if it never reaches it, or only
/// leaves.
fn circle_entry(from: Vec2, motion: Vec2, radius: f32) -> Option<f32> {
    let a = motion.length_squared();
    let c = from.length_squared() - radius * radius;
    if a <= f32::MIN_POSITIVE {
        return (c < 0.0).then_some(f32::NEG_INFINITY);
    }
    let b = from.dot(motion);
    let discriminant = b * b - a * c;
    if discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    let (first, last) = ((-b - root) / a, (-b + root) / a);
    (last > 0.0).then_some(first)
}

fn project(corners: &[Vec2; 4], axis: Vec2) -> (f32, f32) {
    corners
        .iter()
        .map(|c| c.dot(axis))
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        })
}

#[cfg(test)]
mod tests;
