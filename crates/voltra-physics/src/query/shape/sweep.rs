//! The sweeps behind [`super::cast`]: when a moved shape first touches
//! another — in closed form for boxes and circles, by conservative
//! advancement once a capsule or a polygon is in the pair.

use voltra_render::glam::Vec2;
use voltra_scene::{Collider, Transform};

use super::advance::advance;
use super::{PARALLEL, TOUCH};
use crate::geometry::Hull;
use crate::query::{rotated, to_local};

type Shape<'a> = (&'a Collider, &'a Transform);

/// When, as a fraction of `motion`, `a` first touches `b`, and the normal
/// pushing `a` away there. Negative while they start overlapping, with a
/// normal that may be zero if no face is to blame.
pub(super) fn sweep(a: Shape<'_>, motion: Vec2, b: Shape<'_>) -> Option<(f32, Vec2)> {
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
        _ => advance(&Hull::of(a.0, a.1), motion, &Hull::of(b.0, b.1)),
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
