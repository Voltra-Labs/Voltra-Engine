//! Exactly how two shapes overlap, if they do.
//!
//! Three pairs, none of which needs SAT or GJK. Every one of them has a
//! degenerate case where the obvious formula divides by zero or normalises a
//! zero vector, and `glam`'s `normalize` of a zero vector is NaN rather than an
//! error. A NaN normal does not fail here — it spreads into every velocity the
//! solver touches next stage and the scene quietly stops working. So each case
//! is handled where it arises, not guarded for at the end.
//!
//! The normal always pushes **`a` away from `b`**. Swapping the arguments
//! negates it and changes nothing else.

mod box_circle;
mod circle;

use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, Transform};

use box_circle::aabb_circle;
use circle::circle_circle;

/// Below this a difference of positions has no meaningful direction.
const EPSILON: f32 = 1e-6;

/// The direction used when two shapes are exactly concentric.
///
/// Arbitrary, and it has to be *something*: the alternative is a NaN.
const FALLBACK_NORMAL: Vec2 = Vec2::X;

/// One overlap between two entities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    pub a: Entity,
    pub b: Entity,
    /// Unit vector pushing `a` away from `b`.
    pub normal: Vec2,
    /// How deep the overlap is along the normal. Always positive.
    pub penetration: f32,
    /// A point on the overlap, in world space. For drawing now, and for the
    /// torque arm once there is angular velocity.
    pub point: Vec2,
}

/// A shape and where it sits.
type Shape<'a> = (&'a Collider, &'a Transform);

/// The overlap between two shapes, as `(normal, penetration, point)`.
///
/// `None` when they do not overlap, when they merely touch — zero penetration
/// is not a collision, and reporting it would hand the solver a contact with
/// nothing to resolve on every frame two bodies rest against each other — or
/// when either shape has no area.
pub fn contact(a: Shape<'_>, b: Shape<'_>) -> Option<(Vec2, f32, Vec2)> {
    if a.0.is_degenerate(a.1) || b.0.is_degenerate(b.1) {
        return None;
    }

    match (a.0, b.0) {
        (Collider::Circle { .. }, Collider::Circle { .. }) => circle_circle(a, b),
        (Collider::Aabb { .. }, Collider::Aabb { .. }) => aabb_aabb(a, b),
        (Collider::Aabb { .. }, Collider::Circle { .. }) => aabb_circle(a, b),
        (Collider::Circle { .. }, Collider::Aabb { .. }) => {
            // Solved the other way round and mirrored, so there is one
            // implementation of the hard case rather than two that can drift.
            let (normal, penetration, point) = aabb_circle(b, a)?;
            Some((-normal, penetration, point))
        }
    }
}

fn aabb_aabb(a: Shape<'_>, b: Shape<'_>) -> Option<(Vec2, f32, Vec2)> {
    let (ha, hb) = (a.0.world_half_extents(a.1), b.0.world_half_extents(b.1));
    let delta = a.1.translation - b.1.translation;
    let overlap = (ha + hb) - delta.abs();

    if overlap.x <= 0.0 || overlap.y <= 0.0 {
        return None;
    }

    // The axis of *least* penetration: the direction that separates them
    // soonest. Choosing the deeper one pushes a box out through its neighbour
    // instead of off its face.
    let (normal, penetration) = if overlap.x < overlap.y {
        (Vec2::new(sign_away(delta.x), 0.0), overlap.x)
    } else {
        (Vec2::new(0.0, sign_away(delta.y)), overlap.y)
    };

    // On the face of `b` that `a` is pushed away from.
    let point = b.1.translation + normal * (hb * normal.abs()).length();
    Some((normal, penetration, point))
}

/// `+1` or `-1` matching `value`'s sign, and `+1` for exactly zero.
///
/// `f32::signum` returns `-1.0` for `-0.0` and would flip a normal for two
/// bodies at identical coordinates depending on how the subtraction rounded.
fn sign_away(value: f32) -> f32 {
    if value < 0.0 {
        -1.0
    } else {
        1.0
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A transform at a point, shared with the per-pair test modules.
    pub(crate) fn at(x: f32, y: f32) -> Transform {
        Transform::from_translation(Vec2::new(x, y))
    }

    pub(crate) fn circle(r: f32) -> Collider {
        Collider::Circle { radius: r }
    }

    pub(crate) fn boxed(half: f32) -> Collider {
        Collider::Aabb {
            half_extents: Vec2::splat(half),
        }
    }

    #[test]
    fn boxes_report_the_axis_of_least_penetration() {
        // Overlapping 0.2 in x and 1.5 in y: the way out is x, because it is
        // nearer. Choosing the deeper axis pushes a box through its neighbour.
        let (normal, penetration, _) =
            contact((&boxed(1.0), &at(0.0, 0.0)), (&boxed(1.0), &at(1.8, 0.5)))
                .expect("they overlap");

        assert!(
            normal.y.abs() < 1e-6,
            "expected an x normal, got {normal:?}"
        );
        assert!((penetration - 0.2).abs() < 1e-6, "{penetration}");
    }

    #[test]
    fn separated_boxes_do_not_touch() {
        assert!(contact((&boxed(1.0), &at(0.0, 0.0)), (&boxed(1.0), &at(3.0, 0.0))).is_none());
    }

    #[test]
    fn swapping_two_boxes_negates_the_normal() {
        let one =
            contact((&boxed(1.0), &at(0.0, 0.0)), (&boxed(1.0), &at(1.8, 0.5))).expect("overlap");
        let other =
            contact((&boxed(1.0), &at(1.8, 0.5)), (&boxed(1.0), &at(0.0, 0.0))).expect("overlap");

        assert!(
            (one.0 + other.0).length() < 1e-6,
            "{:?} {:?}",
            one.0,
            other.0
        );
    }

    #[test]
    fn a_zero_sized_collider_never_collides() {
        // A scene file can contain a zero or negative radius, and an inverted
        // shape would report a contact with a backwards normal.
        assert!(contact((&circle(0.0), &at(0.0, 0.0)), (&circle(1.0), &at(0.0, 0.0))).is_none());
        assert!(contact(
            (
                &Collider::Aabb {
                    half_extents: Vec2::ZERO
                },
                &at(0.0, 0.0)
            ),
            (&boxed(1.0), &at(0.0, 0.0))
        )
        .is_none());
    }

    #[test]
    fn coincident_boxes_give_a_finite_normal() {
        let (normal, penetration, _) =
            contact((&boxed(1.0), &at(0.0, 0.0)), (&boxed(1.0), &at(0.0, 0.0)))
                .expect("fully overlapping");

        assert!(
            normal.is_finite() && (normal.length() - 1.0).abs() < 1e-6,
            "{normal:?}"
        );
        assert!(penetration > 0.0);
    }

    #[test]
    fn scale_changes_whether_two_bodies_touch() {
        let big = Transform::default().with_scale(Vec2::splat(4.0));

        assert!(contact((&circle(1.0), &big), (&circle(1.0), &at(3.0, 0.0))).is_some());
        assert!(contact(
            (&circle(1.0), &Transform::default()),
            (&circle(1.0), &at(3.0, 0.0))
        )
        .is_none());
    }

    #[test]
    fn every_normal_is_a_unit_vector() {
        // A normal of the wrong length scales every impulse the solver applies
        // by that factor, silently.
        let cases = [
            (circle(1.0), at(0.0, 0.0), circle(1.0), at(1.5, 0.3)),
            (boxed(1.0), at(0.0, 0.0), boxed(1.0), at(1.8, 0.5)),
            (boxed(1.0), at(0.0, 0.0), circle(1.0), at(1.5, 0.7)),
            (circle(1.0), at(1.5, 0.7), boxed(1.0), at(0.0, 0.0)),
        ];

        for (a, at_a, b, at_b) in cases {
            let (normal, _, _) = contact((&a, &at_a), (&b, &at_b)).expect("overlap");
            assert!(
                (normal.length() - 1.0).abs() < 1e-5,
                "{normal:?} has length {}",
                normal.length()
            );
        }
    }
}
