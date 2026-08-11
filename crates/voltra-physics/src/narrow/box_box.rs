//! Box against box: the separating axis, then clipping one face to the other.
//!
//! This is the pair that needs two contact points. A box resting on a floor
//! touches it along a whole face, and a solver given the single deepest point
//! corrects one corner per step and rocks the box forever. Clipping the
//! incident face against the reference face is what produces the second point.
//!
//! The algorithm is Box2D v3's `b2CollidePolygons`, over four vertices instead
//! of a general polygon: find the axis of least penetration over both boxes'
//! face normals, pick a reference face, clip, and keep what is still below the
//! face. Written against a vertex list rather than half extents, so the convex
//! polygon of a later stage is a different caller, not a different algorithm.

use voltra_render::glam::Vec2;

use super::{Manifold, ManifoldPoint, Shape};

/// How much better B's axis must be before it becomes the reference.
///
/// Box2D's `0.1 · linear_slop`, and the reason is warm starting: two axes that
/// separate by the same amount let the reference flip between steps on a
/// resting box, which changes every point id and throws away every impulse the
/// contact had accumulated. A hair of hysteresis keeps the choice made.
const FLIP_BIAS: f32 = 5e-4;

/// Below this an interval is too short to interpolate along.
const EPSILON: f32 = 1e-9;

pub(super) fn box_box(a: Shape<'_>, b: Shape<'_>) -> Option<Manifold> {
    let corners_a = a.0.corners(a.1);
    let corners_b = b.0.corners(b.1);

    // Separating axis: only the two boxes' own face normals can separate them,
    // so eight half-tests, of which four are each box's mirrored faces.
    let (edge_a, separation_a) = max_separation(&corners_a, &corners_b);
    if separation_a >= 0.0 {
        return None;
    }
    let (edge_b, separation_b) = max_separation(&corners_b, &corners_a);
    if separation_b >= 0.0 {
        return None;
    }

    let flip = separation_b > separation_a + FLIP_BIAS;
    let (reference, incident, reference_edge) = if flip {
        (&corners_b, &corners_a, edge_b)
    } else {
        (&corners_a, &corners_b, edge_a)
    };

    Some(clip(reference, incident, reference_edge, flip))
}

/// The best separating axis among `reference`'s faces, as `(edge, separation)`.
///
/// Negative while the boxes overlap. The edge is the one whose outward normal
/// leaves the least overlap — the direction that separates them soonest, which
/// is the face a body should slide off rather than be pushed through.
fn max_separation(reference: &[Vec2; 4], incident: &[Vec2; 4]) -> (usize, f32) {
    let mut best_edge = 0;
    let mut best = f32::NEG_INFINITY;

    for edge in 0..4 {
        let normal = edge_normal(reference, edge);
        let separation = incident
            .iter()
            .map(|corner| normal.dot(*corner - reference[edge]))
            .fold(f32::INFINITY, f32::min);

        if separation > best {
            best = separation;
            best_edge = edge;
        }
    }

    (best_edge, best)
}

/// The outward unit normal of the edge leaving corner `edge`.
///
/// The corners wind counter-clockwise, so the outward normal of `p → q` is
/// `(d.y, -d.x)`. A zero-length edge cannot happen for a collider that passed
/// `is_degenerate`, and `normalize_or_zero` keeps it a zero rather than a NaN
/// if one ever does.
fn edge_normal(polygon: &[Vec2; 4], edge: usize) -> Vec2 {
    let direction = polygon[(edge + 1) % 4] - polygon[edge];
    Vec2::new(direction.y, -direction.x).normalize_or_zero()
}

/// The incident face: the one most opposed to the reference normal.
fn incident_edge(incident: &[Vec2; 4], reference_normal: Vec2) -> usize {
    let mut best_edge = 0;
    let mut best = f32::INFINITY;

    for edge in 0..4 {
        let alignment = edge_normal(incident, edge).dot(reference_normal);
        if alignment < best {
            best = alignment;
            best_edge = edge;
        }
    }

    best_edge
}

/// Clips the incident face to the reference face's side planes.
fn clip(
    reference: &[Vec2; 4],
    incident: &[Vec2; 4],
    reference_edge: usize,
    flip: bool,
) -> Manifold {
    let normal = edge_normal(reference, reference_edge);
    let (i11, i12) = (reference_edge, (reference_edge + 1) % 4);
    let (v11, v12) = (reference[i11], reference[i12]);

    let incident_edge = incident_edge(incident, normal);
    let (i21, i22) = (incident_edge, (incident_edge + 1) % 4);
    let (v21, v22) = (incident[i21], incident[i22]);

    // Along the reference face. The incident face runs the other way, which is
    // why `v22` is the lower end and `v21` the upper one.
    let tangent = Vec2::new(-normal.y, normal.x);
    let (lower1, upper1) = (0.0, tangent.dot(v12 - v11));
    let lower2 = tangent.dot(v22 - v11);
    let upper2 = tangent.dot(v21 - v11);
    let span = upper2 - lower2;

    let lower = if lower2 < lower1 && span > EPSILON {
        v22.lerp(v21, (lower1 - lower2) / span)
    } else {
        v22
    };
    let upper = if upper2 > upper1 && span > EPSILON {
        v22.lerp(v21, (upper1 - lower2) / span)
    } else {
        v21
    };

    // An id names the pair of features that produced the point: the reference
    // corner that cut it and the incident corner it came from. Built after the
    // flip, so the same physical corner keeps its id whichever box was the
    // reference and warm starting survives the swap.
    let ids = if flip {
        [make_id(i22, i11), make_id(i21, i12)]
    } else {
        [make_id(i11, i22), make_id(i12, i21)]
    };

    let candidates = [
        ManifoldPoint {
            point: lower,
            separation: normal.dot(lower - v11),
            id: ids[0],
        },
        ManifoldPoint {
            point: upper,
            separation: normal.dot(upper - v11),
            id: ids[1],
        },
    ];

    let touching: Vec<ManifoldPoint> = candidates
        .iter()
        .copied()
        .filter(|point| point.separation < 0.0)
        .collect();

    // The separating axis said these boxes overlap, so the manifold must have
    // a point even when clipping leaves both ends of the incident face outside
    // — a corner contact caught between the two side planes. The deeper end is
    // then the contact, and its separation is a hair above zero, which the
    // solver already treats as a contact about to close.
    let points = if touching.is_empty() {
        let deeper = if candidates[0].separation <= candidates[1].separation {
            candidates[0]
        } else {
            candidates[1]
        };
        vec![deeper]
    } else {
        touching
    };

    // Our normal pushes `a` away from `b`. A reference face on `a` has its
    // outward normal pointing at `b`, so that case is the one that flips.
    Manifold::new(if flip { normal } else { -normal }, &points)
}

/// Two feature indices packed into one id, as Box2D's `B2_MAKE_ID` does.
fn make_id(reference_corner: usize, incident_corner: usize) -> u16 {
    ((reference_corner as u16) << 8) | incident_corner as u16
}

#[cfg(test)]
mod tests {
    use crate::narrow::manifold;
    use crate::narrow::tests::at;
    use std::f32::consts::FRAC_PI_4;
    use voltra_render::glam::Vec2;
    use voltra_scene::{Collider, Transform};

    /// A wide, thin floor whose top face is at y = 0.
    fn floor() -> Collider {
        Collider::Box {
            half_extents: Vec2::new(5.0, 1.0),
        }
    }

    fn floor_at() -> Transform {
        Transform::from_translation(Vec2::new(0.0, -1.0))
    }

    /// A unit box, half extents 0.5.
    fn unit() -> Collider {
        Collider::Box {
            half_extents: Vec2::splat(0.5),
        }
    }

    fn unit_at(y: f32, rotation: f32) -> Transform {
        Transform::from_translation(Vec2::new(0.0, y)).with_rotation(rotation)
    }

    #[test]
    fn two_boxes_resting_face_to_face_give_two_points() {
        // The box's bottom face is 0.1 below the floor's top face.
        let m =
            manifold((&floor(), &floor_at()), (&unit(), &unit_at(0.4, 0.0))).expect("they overlap");

        assert_eq!(m.points().len(), 2, "{m:?}");
        assert!((m.normal - Vec2::NEG_Y).length() < 1e-5, "{m:?}");
        for point in m.points() {
            assert!((point.separation + 0.1).abs() < 1e-4, "{point:?}");
        }
    }

    #[test]
    fn the_two_points_are_the_ends_of_the_overlapping_face() {
        let m =
            manifold((&floor(), &floor_at()), (&unit(), &unit_at(0.4, 0.0))).expect("they overlap");

        let xs: Vec<f32> = m.points().iter().map(|point| point.point.x).collect();
        assert!(xs.iter().any(|x| (*x + 0.5).abs() < 1e-4), "{xs:?}");
        assert!(xs.iter().any(|x| (*x - 0.5).abs() < 1e-4), "{xs:?}");
    }

    #[test]
    fn a_corner_contact_gives_one_point() {
        // Turned 45°, the box reaches down to its corner at -√2/2 ≈ -0.707.
        let m = manifold(
            (&floor(), &floor_at()),
            (&unit(), &unit_at(0.65, FRAC_PI_4)),
        )
        .expect("the corner is through the floor");

        assert_eq!(m.points().len(), 1, "{m:?}");
        assert!((m.normal - Vec2::NEG_Y).length() < 1e-5, "{m:?}");
    }

    #[test]
    fn separated_boxes_report_nothing() {
        assert!(manifold((&floor(), &floor_at()), (&unit(), &unit_at(2.0, 0.0))).is_none());
        assert!(manifold((&floor(), &floor_at()), (&unit(), &unit_at(2.0, FRAC_PI_4))).is_none());
    }

    #[test]
    fn touching_exactly_is_not_a_collision() {
        // Zero penetration hands the solver a contact with nothing to resolve,
        // on every frame two bodies rest against each other.
        assert!(manifold((&floor(), &floor_at()), (&unit(), &unit_at(0.5, 0.0))).is_none());
    }

    #[test]
    fn the_normal_always_pushes_a_away_from_b() {
        let one =
            manifold((&floor(), &floor_at()), (&unit(), &unit_at(0.4, 0.0))).expect("they overlap");
        let other =
            manifold((&unit(), &unit_at(0.4, 0.0)), (&floor(), &floor_at())).expect("they overlap");

        assert!((one.normal - Vec2::NEG_Y).length() < 1e-5, "{one:?}");
        assert!((other.normal - Vec2::Y).length() < 1e-5, "{other:?}");
    }

    #[test]
    fn the_point_ids_are_stable_while_the_features_are() {
        let ids = |transform: &Transform| {
            manifold((&floor(), &floor_at()), (&unit(), transform))
                .expect("they overlap")
                .points()
                .iter()
                .map(|point| point.id)
                .collect::<Vec<_>>()
        };

        assert_eq!(ids(&unit_at(0.4, 0.0)), ids(&unit_at(0.399, 0.0)));
    }

    #[test]
    fn the_two_points_of_a_manifold_have_different_ids() {
        let m =
            manifold((&floor(), &floor_at()), (&unit(), &unit_at(0.4, 0.0))).expect("they overlap");

        assert_ne!(m.points()[0].id, m.points()[1].id, "{m:?}");
    }

    #[test]
    fn boxes_report_the_axis_of_least_penetration() {
        // Overlapping 0.2 in x and 1.5 in y: the way out is x, because it is
        // nearer. Choosing the deeper axis pushes a box through its neighbour.
        let m = manifold((&unit(), &at(0.0, 0.0)), (&unit(), &at(0.8, 0.5))).expect("they overlap");

        assert!(
            m.normal.y.abs() < 1e-5,
            "expected an x normal, got {:?}",
            m.normal
        );
        assert!((m.deepest_separation() + 0.2).abs() < 1e-4, "{m:?}");
    }

    #[test]
    fn a_reference_face_does_not_flip_between_two_near_equal_axes() {
        // A square overlapping a square by the same amount on both axes: the
        // two candidate references separate equally, and a solver that changes
        // its mind each step throws away its warm start.
        let first = manifold((&unit(), &at(0.0, 0.0)), (&unit(), &at(0.6, 0.6))).expect("overlap");
        let second =
            manifold((&unit(), &at(0.0, 0.0)), (&unit(), &at(0.6, 0.600001))).expect("overlap");

        assert!(
            (first.normal - second.normal).length() < 1e-3,
            "{first:?} {second:?}"
        );
    }

    #[test]
    fn a_rotated_box_meets_a_rotated_floor_squarely() {
        // Both turned by the same angle: the contact is still face to face, so
        // it is still two points, and the normal is the turned floor's.
        let angle = 0.3;
        let floor_transform =
            Transform::from_translation(Vec2::new(0.0, -1.0)).with_rotation(angle);
        let rotation = Vec2::from_angle(angle);
        let box_transform = Transform::from_translation(
            floor_transform.translation + rotation.rotate(Vec2::new(0.0, 1.4)),
        )
        .with_rotation(angle);

        let m = manifold((&floor(), &floor_transform), (&unit(), &box_transform))
            .expect("they overlap");

        assert_eq!(m.points().len(), 2, "{m:?}");
        assert!(
            (m.normal + rotation.rotate(Vec2::Y)).length() < 1e-4,
            "{m:?}"
        );
    }

    #[test]
    fn coincident_boxes_give_a_finite_normal() {
        let m = manifold((&unit(), &at(0.0, 0.0)), (&unit(), &at(0.0, 0.0)))
            .expect("fully overlapping");

        assert!(
            m.normal.is_finite() && (m.normal.length() - 1.0).abs() < 1e-5,
            "{m:?}"
        );
        assert!(m.deepest_separation() < 0.0, "{m:?}");
        assert!(!m.points().is_empty(), "{m:?}");
    }

    #[test]
    fn a_deeply_buried_box_still_reports_one_normal() {
        // A small box entirely inside a large one: every axis overlaps, and the
        // least of them is the answer rather than a NaN.
        let big = Collider::Box {
            half_extents: Vec2::splat(4.0),
        };
        let m = manifold((&big, &at(0.0, 0.0)), (&unit(), &at(1.0, 0.0))).expect("inside");

        assert!((m.normal.length() - 1.0).abs() < 1e-5, "{m:?}");
        assert!(!m.points().is_empty(), "{m:?}");
    }
}
