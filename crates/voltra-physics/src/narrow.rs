//! Exactly how two shapes overlap, if they do.
//!
//! Every shape is a convex hull grown by a radius (`Collider2D::world_shape`),
//! so the general pairs are by kind, as in Box2D v3: polygon against polygon
//! — a capsule is a two-cornered one — by the separating axis and clipping
//! (`polygon`), anything against a circle by its nearest feature
//! (`round`), and capsule against capsule by the closest points of their
//! segments (`capsule`). Box against box and box against circle keep
//! closed-form paths of their own: the same answers, measured at 114 against
//! 284 ns a box pair and 28 against 122 a box–circle pair.
//!
//! Every pair has a degenerate case where the obvious formula divides by zero
//! or normalises a zero vector, and `glam`'s `normalize` of a zero vector is
//! NaN rather than an error. A NaN normal does not fail here — it spreads into
//! every velocity the solver touches next stage and the scene quietly stops
//! working. So each case is handled where it arises, not guarded for at the
//! end.
//!
//! The normal always pushes **`a` away from `b`**. Swapping the arguments
//! negates it and changes nothing else.

mod box_box;
mod box_circle;
mod capsule;
mod circle;
mod polygon;
mod round;

use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{Collider2D, Transform2D};

use crate::geometry::Hull;
use box_box::box_box;
use box_circle::box_circle;
use capsule::capsule_capsule;
use circle::circle_circle;
use polygon::polygon_polygon;
use round::{hull_circle, point_point};

/// Below this a difference of positions has no meaningful direction.
const EPSILON: f32 = 1e-6;

/// The direction used when two shapes are exactly concentric.
///
/// Arbitrary, and it has to be *something*: the alternative is a NaN.
const FALLBACK_NORMAL: Vec2 = Vec2::X;

/// Where two shapes touch, and how far into each other they are.
///
/// `separation` is negative while they overlap, which is the solver's sign
/// rather than the narrow phase's: the sub-steps track a separation upwards
/// towards zero, and flipping the sign once here beats flipping it at every
/// use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ManifoldPoint {
    /// World space, on the overlap.
    pub point: Vec2,
    /// Negative while the shapes overlap.
    pub separation: f32,
    /// Which pair of features produced this point.
    ///
    /// The warm-start key: the same physical corner must carry the same id
    /// from one step to the next or its impulse is thrown away and the contact
    /// starts cold. Shapes with a single feature — every circle — use `0`.
    pub id: u16,
}

/// One overlap, as one normal and up to two points.
///
/// Two is the bound because two convex faces can only cross at two points, and
/// nothing here produces a third. It is Box2D's bound for the same reason, and
/// an array of two costs no allocation where a `Vec` would cost one per
/// contact per step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Manifold {
    /// Unit vector pushing `a` away from `b`. One normal for both points.
    pub normal: Vec2,
    points: [ManifoldPoint; 2],
    count: u8,
}

impl Manifold {
    /// A manifold of the first two of `points`.
    ///
    /// The only constructor, so `count` cannot disagree with the array. More
    /// than two points is a bug in a pair function rather than a case to
    /// handle, and taking the first two keeps a wrong manifold solvable
    /// instead of turning it into a panic in the middle of a step.
    pub(crate) fn new(normal: Vec2, points: &[ManifoldPoint]) -> Self {
        debug_assert!(points.len() <= 2, "a manifold holds at most two points");
        let mut stored = [points[0]; 2];
        let count = points.len().min(2);
        stored[..count].copy_from_slice(&points[..count]);
        Self {
            normal,
            points: stored,
            count: count as u8,
        }
    }

    /// The points this manifold actually has.
    pub fn points(&self) -> &[ManifoldPoint] {
        &self.points[..self.count as usize]
    }

    /// The most negative separation of its points.
    pub fn deepest_separation(&self) -> f32 {
        self.points()
            .iter()
            .map(|point| point.separation)
            .fold(f32::INFINITY, f32::min)
    }

    /// The same manifold with its points moved by `offset`.
    pub(crate) fn moved(mut self, offset: Vec2) -> Self {
        for point in &mut self.points[..self.count as usize] {
            point.point += offset;
        }
        self
    }

    /// The same manifold with the normal reversed, for the mirrored argument
    /// order. The points do not move: they are already where the shapes meet.
    pub(crate) fn flipped(self) -> Self {
        Self {
            normal: -self.normal,
            ..self
        }
    }
}

/// One overlap between two entities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    /// One entity of the overlap.
    pub a: Entity,
    /// The other entity.
    pub b: Entity,
    /// Where and how deeply they overlap.
    pub manifold: Manifold,
}

impl Contact {
    /// A contact between `a` and `b` described by `manifold`.
    pub fn new(a: Entity, b: Entity, manifold: Manifold) -> Self {
        Self { a, b, manifold }
    }

    /// Unit vector pushing `a` away from `b`.
    pub fn normal(&self) -> Vec2 {
        self.manifold.normal
    }

    /// The contact points of the manifold.
    pub fn points(&self) -> &[ManifoldPoint] {
        self.manifold.points()
    }

    /// The most negative separation of its points — how deep the overlap is at
    /// its worst.
    pub fn deepest(&self) -> f32 {
        self.manifold.deepest_separation()
    }
}

/// A shape and where it sits.
type Shape<'a> = (&'a Collider2D, &'a Transform2D);

/// The overlap between two shapes.
///
/// `None` when they do not overlap, when they merely touch — zero penetration
/// is not a collision, and reporting it would hand the solver a contact with
/// nothing to resolve on every frame two bodies rest against each other — or
/// when either shape has no area.
pub fn manifold(a: Shape<'_>, b: Shape<'_>) -> Option<Manifold> {
    if a.0.is_degenerate(a.1) || b.0.is_degenerate(b.1) {
        return None;
    }

    match (a.0, b.0) {
        (Collider2D::Circle { .. }, Collider2D::Circle { .. }) => circle_circle(a, b),
        (Collider2D::Box { .. }, Collider2D::Box { .. }) => box_box(a, b),
        (Collider2D::Box { .. }, Collider2D::Circle { .. }) => box_circle(a, b),
        (Collider2D::Circle { .. }, Collider2D::Box { .. }) => {
            // Solved the other way round and mirrored, so there is one
            // implementation of the hard case rather than two that can drift.
            Some(box_circle(b, a)?.flipped())
        }
        _ => hulls(&Hull::of(a.0, a.1), &Hull::of(b.0, b.1)),
    }
}

/// The contact two shapes *apart* will make if they close, measured as
/// though each were grown by half of `margin`: points on the features that
/// face each other, with positive separations — the gaps there.
///
/// What a fast body is given ahead of the solve (`continuous`): the solver
/// lets a contact of positive separation close exactly that far and no
/// further, which is a rigid stop where an overlap's soft one would let a
/// fast body sink in. `None` when the gap is wider than `margin`.
pub(crate) fn speculative(a: Shape<'_>, b: Shape<'_>, margin: f32) -> Option<Manifold> {
    if a.0.is_degenerate(a.1) || b.0.is_degenerate(b.1) || margin.is_nan() || margin <= 0.0 {
        return None;
    }
    let (mut grown_a, mut grown_b) = (Hull::of(a.0, a.1), Hull::of(b.0, b.1));
    grown_a.radius += 0.5 * margin;
    grown_b.radius += 0.5 * margin;
    let mut manifold = hulls(&grown_a, &grown_b)?;
    for point in &mut manifold.points[..manifold.count as usize] {
        point.separation += margin;
    }
    Some(manifold)
}

/// Any pair, as two placed hulls: by how many corners each has, since a
/// capsule as short as it is wide is a circle and collides as one.
fn hulls(a: &Hull, b: &Hull) -> Option<Manifold> {
    match (a.len(), b.len()) {
        (1, 1) => point_point(a.vertices()[0], a.radius, b.vertices()[0], b.radius),
        (_, 1) => hull_circle(a, b.vertices()[0], b.radius),
        (1, _) => Some(hull_circle(b, a.vertices()[0], a.radius)?.flipped()),
        (2, 2) => capsule_capsule(a, b),
        _ => polygon_polygon(a, b),
    }
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
    pub(crate) fn at(x: f32, y: f32) -> Transform2D {
        Transform2D::from_translation(Vec2::new(x, y))
    }

    pub(crate) fn circle(r: f32) -> Collider2D {
        Collider2D::Circle { radius: r }
    }

    pub(crate) fn boxed(half: f32) -> Collider2D {
        Collider2D::Box {
            half_extents: Vec2::splat(half),
        }
    }

    #[test]
    fn swapping_two_boxes_negates_the_normal() {
        let one =
            manifold((&boxed(1.0), &at(0.0, 0.0)), (&boxed(1.0), &at(1.8, 0.5))).expect("overlap");
        let other =
            manifold((&boxed(1.0), &at(1.8, 0.5)), (&boxed(1.0), &at(0.0, 0.0))).expect("overlap");

        assert!(
            (one.normal + other.normal).length() < 1e-5,
            "{:?} {:?}",
            one.normal,
            other.normal
        );
    }

    #[test]
    fn a_zero_sized_collider_never_collides() {
        // A scene file can contain a zero or negative radius, and an inverted
        // shape would report a contact with a backwards normal.
        assert!(manifold((&circle(0.0), &at(0.0, 0.0)), (&circle(1.0), &at(0.0, 0.0))).is_none());
        assert!(manifold(
            (
                &Collider2D::Box {
                    half_extents: Vec2::ZERO
                },
                &at(0.0, 0.0)
            ),
            (&boxed(1.0), &at(0.0, 0.0))
        )
        .is_none());
    }

    #[test]
    fn scale_changes_whether_two_bodies_touch() {
        let big = Transform2D::default().with_scale(Vec2::splat(4.0));

        assert!(manifold((&circle(1.0), &big), (&circle(1.0), &at(3.0, 0.0))).is_some());
        assert!(manifold(
            (&circle(1.0), &Transform2D::default()),
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
            let m = manifold((&a, &at_a), (&b, &at_b)).expect("overlap");
            assert!(
                (m.normal.length() - 1.0).abs() < 1e-5,
                "{:?} has length {}",
                m.normal,
                m.normal.length()
            );
        }
    }

    #[test]
    fn a_manifold_holds_at_most_two_points() {
        let m = manifold((&boxed(1.0), &at(0.0, 0.0)), (&boxed(1.0), &at(1.8, 0.5)))
            .expect("they overlap");

        assert!(m.points().len() <= 2, "{m:?}");
        assert!(
            !m.points().is_empty(),
            "a manifold with no points is not one"
        );
    }

    #[test]
    fn a_contact_reports_its_deepest_point() {
        let m = Manifold::new(
            Vec2::Y,
            &[
                ManifoldPoint {
                    point: Vec2::ZERO,
                    separation: -0.1,
                    id: 1,
                },
                ManifoldPoint {
                    point: Vec2::X,
                    separation: -0.4,
                    id: 2,
                },
            ],
        );
        let mut world = voltra_ecs::World::new();
        let contact = Contact::new(world.spawn(), world.spawn(), m);

        assert_eq!(contact.points().len(), 2);
        assert!((contact.deepest() + 0.4).abs() < 1e-6);
        assert_eq!(contact.normal(), Vec2::Y);
    }
}
