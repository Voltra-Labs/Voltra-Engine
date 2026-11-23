//! Polygon against polygon, any corner count from two, rounded or sharp:
//! Box2D v3's `b2CollidePolygons`.
//!
//! The separating axis over both shapes' face normals finds the reference
//! face; while the cores overlap, the incident face is clipped against the
//! reference face's side planes for up to two points, as for two boxes
//! (`box_box`). Once only the rounding overlaps — the cores apart — the two
//! faces' closest points decide: corner against corner is one point along
//! the line between them, anything else is clipped as before. Separations
//! subtract both radii, and each point sits midway between the surfaces.
//!
//! A capsule is a two-cornered polygon here, which is what Box2D makes of
//! one against a polygon too.

use voltra_render::glam::Vec2;

use super::{Manifold, ManifoldPoint};
use crate::geometry::{segment_distance, Hull};

/// How much better B's axis must be before it becomes the reference: the
/// hysteresis that keeps a resting contact's ids, as in `box_box`.
const FLIP_BIAS: f32 = 5e-4;

/// Cores further apart than this are apart, and their closest points are
/// far enough apart to give a direction: Box2D's `0.1 · linear_slop`.
const CORE_SLOP: f32 = 5e-4;

/// Below this an interval is too short to interpolate along.
const EPSILON: f32 = 1e-9;

pub(super) fn polygon_polygon(a: &Hull, b: &Hull) -> Option<Manifold> {
    // Worked near the origin, where `f32` has the most digits: a level a
    // long way from zero loses them on every subtraction otherwise.
    let origin = a.vertices()[0];
    let (a, b) = (a.moved(-origin), b.moved(-origin));

    let (edge_a, separation_a) = max_separation(&a, &b);
    let (edge_b, separation_b) = max_separation(&b, &a);
    let radius = a.radius + b.radius;
    if separation_a >= radius || separation_b >= radius {
        return None;
    }

    let flip = separation_b > separation_a + FLIP_BIAS;
    let (edge_a, edge_b) = if flip {
        (incident_edge(&a, b.normals()[edge_b]), edge_b)
    } else {
        (edge_a, incident_edge(&b, a.normals()[edge_a]))
    };

    let manifold = if separation_a > CORE_SLOP || separation_b > CORE_SLOP {
        apart(&a, &b, edge_a, edge_b, flip)
    } else {
        clip(&a, &b, edge_a, edge_b, flip)
    }?;
    Some(manifold.moved(origin))
}

/// The best separating axis among `reference`'s faces, `(edge, separation)`,
/// between the cores.
fn max_separation(reference: &Hull, incident: &Hull) -> (usize, f32) {
    let mut best = (0, f32::NEG_INFINITY);
    for (edge, (normal, vertex)) in reference
        .normals()
        .iter()
        .zip(reference.vertices())
        .enumerate()
    {
        let separation = incident
            .vertices()
            .iter()
            .map(|v| normal.dot(*v - *vertex))
            .fold(f32::INFINITY, f32::min);
        if separation > best.1 {
            best = (edge, separation);
        }
    }
    best
}

/// The face of `incident` most opposed to `normal`.
fn incident_edge(incident: &Hull, normal: Vec2) -> usize {
    let mut best = (0, f32::INFINITY);
    for (edge, n) in incident.normals().iter().enumerate() {
        let alignment = n.dot(normal);
        if alignment < best.1 {
            best = (edge, alignment);
        }
    }
    best.0
}

/// The cores are apart and only their rounding overlaps: the closest points
/// of the two faces decide.
fn apart(a: &Hull, b: &Hull, edge_a: usize, edge_b: usize, flip: bool) -> Option<Manifold> {
    let (i11, i12) = (edge_a, a.next(edge_a));
    let (i21, i22) = (edge_b, b.next(edge_b));
    let (v11, v12) = (a.vertices()[i11], a.vertices()[i12]);
    let (v21, v22) = (b.vertices()[i21], b.vertices()[i22]);
    let closest = segment_distance(v11, v12, v21, v22);

    let at_end = |f: f32| f == 0.0 || f == 1.0;
    if !(at_end(closest.fraction1) && at_end(closest.fraction2)) {
        return clip(a, b, edge_a, edge_b, flip);
    }
    // Corner against corner: one point, along the line between them.
    let corner_a = if closest.fraction1 == 0.0 { i11 } else { i12 };
    let corner_b = if closest.fraction2 == 0.0 { i21 } else { i22 };
    let distance = closest.distance_squared.sqrt();
    let separation = distance - (a.radius + b.radius);
    if separation >= 0.0 || distance <= 0.0 {
        return None;
    }
    let towards_b = (closest.closest2 - closest.closest1) / distance;
    let surface_a = closest.closest1 + towards_b * a.radius;
    let surface_b = closest.closest2 - towards_b * b.radius;
    Some(Manifold::new(
        -towards_b,
        &[ManifoldPoint {
            point: surface_a.lerp(surface_b, 0.5),
            separation,
            id: make_id(corner_a, corner_b),
        }],
    ))
}

/// Clips the incident face to the reference face's side planes.
fn clip(a: &Hull, b: &Hull, edge_a: usize, edge_b: usize, flip: bool) -> Option<Manifold> {
    let (reference, incident, i11, i21) = if flip {
        (b, a, edge_b, edge_a)
    } else {
        (a, b, edge_a, edge_b)
    };
    let (i12, i22) = (reference.next(i11), incident.next(i21));
    let normal = reference.normals()[i11];
    let (v11, v12) = (reference.vertices()[i11], reference.vertices()[i12]);
    let (v21, v22) = (incident.vertices()[i21], incident.vertices()[i22]);

    // Along the reference face. The incident face runs the other way, which
    // is why `v22` is the lower end and `v21` the upper one.
    let tangent = normal.perp();
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

    let radius = reference.radius + incident.radius;
    let ids = if flip {
        [make_id(i22, i11), make_id(i21, i12)]
    } else {
        [make_id(i11, i22), make_id(i12, i21)]
    };
    let candidates = [(lower, ids[0]), (upper, ids[1])].map(|(point, id)| {
        let between = normal.dot(point - v11);
        ManifoldPoint {
            // Midway between the reference surface and the incident one.
            point: point + normal * (0.5 * (reference.radius - incident.radius - between)),
            separation: between - radius,
            id,
        }
    });

    let mut points = [candidates[0]; 2];
    let mut count = 0;
    for point in candidates {
        if point.separation < 0.0 {
            points[count] = point;
            count += 1;
        }
    }
    if count == 0 {
        // The axis test said they overlap, so there is a contact even when
        // clipping leaves both ends just outside: the deeper one.
        points[0] = if candidates[0].separation <= candidates[1].separation {
            candidates[0]
        } else {
            candidates[1]
        };
        count = 1;
    }
    // A reference face on `a` points at `b`; away from `b` is its reverse.
    Some(Manifold::new(
        if flip { normal } else { -normal },
        &points[..count],
    ))
}

/// Two feature indices packed into one id, as Box2D's `B2_MAKE_ID` does.
fn make_id(reference_corner: usize, incident_corner: usize) -> u16 {
    ((reference_corner as u16) << 8) | incident_corner as u16
}

#[cfg(test)]
mod tests {
    use crate::narrow::manifold;
    use crate::narrow::tests::{at, boxed};
    use voltra_render::glam::Vec2;
    use voltra_scene::{Collider, ConvexPolygon, Transform};

    fn polygon(points: &[(f32, f32)], radius: f32) -> Collider {
        let points: Vec<Vec2> = points.iter().map(|(x, y)| Vec2::new(*x, *y)).collect();
        Collider::Polygon {
            points: ConvexPolygon::new(&points).expect("a polygon"),
            radius,
        }
    }

    fn square_polygon(half: f32) -> Collider {
        Collider::Polygon {
            points: ConvexPolygon::rectangle(Vec2::splat(half)).expect("a square"),
            radius: 0.0,
        }
    }

    /// A wide floor whose top face is at y = 0, as a polygon.
    fn floor() -> (Collider, Transform) {
        (
            polygon(&[(-5.0, -1.0), (5.0, -1.0), (5.0, 0.0), (-5.0, 0.0)], 0.0),
            Transform::default(),
        )
    }

    #[test]
    fn a_box_resting_on_a_polygon_floor_gives_two_points() {
        let (floor, floor_at) = floor();
        let m = manifold((&boxed(0.5), &at(0.0, 0.4)), (&floor, &floor_at)).expect("overlap");

        assert_eq!(m.points().len(), 2, "{m:?}");
        assert!((m.normal - Vec2::Y).length() < 1e-5, "{m:?}");
        for point in m.points() {
            assert!((point.separation + 0.1).abs() < 1e-4, "{point:?}");
        }
    }

    #[test]
    fn polygons_answer_as_boxes_do_wherever_they_meet() {
        // The box pair's fast path and the general code are one algorithm;
        // a disagreement is a bug in one of them.
        let mut seed = 7_u32;
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / (1 << 24) as f32
        };
        let mut compared = 0;
        for _ in 0..400 {
            let b_at =
                Transform::from_translation(Vec2::new(next() * 3.0 - 1.5, next() * 3.0 - 1.5))
                    .with_rotation(next() * std::f32::consts::TAU);
            let a_at = Transform::default().with_rotation(next() * std::f32::consts::TAU);
            let as_boxes = manifold((&boxed(1.0), &a_at), (&boxed(0.7), &b_at));
            let as_polygons =
                manifold((&square_polygon(1.0), &a_at), (&square_polygon(0.7), &b_at));
            match (as_boxes, as_polygons) {
                (None, None) => {}
                (Some(x), Some(y)) => {
                    compared += 1;
                    assert!((x.normal - y.normal).length() < 1e-3, "{x:?} {y:?}");
                    assert!(
                        (x.deepest_separation() - y.deepest_separation()).abs() < 1e-3,
                        "{x:?} {y:?}"
                    );
                    assert_eq!(x.points().len(), y.points().len(), "{x:?} {y:?}");
                }
                (x, y) => panic!("one overlaps and one does not: {x:?} {y:?}"),
            }
        }
        assert!(compared > 100, "{compared} overlapping cases");
    }

    #[test]
    fn a_rounded_polygon_counts_its_rounding() {
        let (floor, floor_at) = floor();
        // A unit square rounded by 0.25 whose core's bottom is 0.15 up.
        let rounded = Collider::Polygon {
            points: ConvexPolygon::rectangle(Vec2::splat(0.5)).expect("a square"),
            radius: 0.25,
        };
        let m = manifold((&rounded, &at(0.0, 0.65)), (&floor, &floor_at)).expect("overlap");

        assert_eq!(m.points().len(), 2, "{m:?}");
        for point in m.points() {
            assert!((point.separation + 0.1).abs() < 1e-4, "{point:?}");
            // Midway between the two surfaces.
            assert!((point.point.y + 0.05).abs() < 1e-4, "{point:?}");
        }
    }

    #[test]
    fn a_rounded_corner_over_a_corner_is_one_point_along_the_diagonal() {
        let a = Collider::Polygon {
            points: ConvexPolygon::rectangle(Vec2::splat(0.5)).expect("a square"),
            radius: 0.5,
        };
        // Cores' corners 0.3·√2 ≈ 0.42 apart, closer than the rounding.
        let m = manifold((&a, &at(1.3, 1.3)), (&boxed(0.5), &at(0.0, 0.0))).expect("overlap");

        assert_eq!(m.points().len(), 1, "{m:?}");
        let diagonal = Vec2::ONE.normalize();
        assert!((m.normal - diagonal).length() < 1e-4, "{m:?}");
        assert!(
            (m.points()[0].separation - (0.3 * 2f32.sqrt() - 0.5)).abs() < 1e-4,
            "{m:?}"
        );
    }

    #[test]
    fn a_capsule_lying_on_a_floor_touches_it_twice_and_standing_once() {
        let (floor, floor_at) = floor();
        let capsule = Collider::Capsule {
            half_height: 1.0,
            radius: 0.25,
        };
        let lying = Transform::from_translation(Vec2::new(0.0, 0.2))
            .with_rotation(std::f32::consts::FRAC_PI_2);
        let m = manifold((&capsule, &lying), (&floor, &floor_at)).expect("lying");
        assert_eq!(m.points().len(), 2, "{m:?}");
        assert!((m.normal - Vec2::Y).length() < 1e-5, "{m:?}");
        for point in m.points() {
            assert!((point.separation + 0.05).abs() < 1e-4, "{point:?}");
        }

        let m = manifold((&capsule, &at(0.0, 0.9)), (&floor, &floor_at)).expect("standing");
        assert_eq!(m.points().len(), 1, "{m:?}");
        assert!((m.points()[0].separation + 0.1).abs() < 1e-4, "{m:?}");
        assert!((m.normal - Vec2::Y).length() < 1e-5, "{m:?}");
    }

    #[test]
    fn apart_polygons_do_not_touch() {
        let triangle = polygon(&[(-1.0, 0.0), (1.0, 0.0), (0.0, 1.0)], 0.0);
        assert!(manifold((&triangle, &at(0.0, 0.0)), (&triangle, &at(0.0, 1.01))).is_none());
        assert!(manifold((&triangle, &at(0.0, 0.0)), (&triangle, &at(2.0, 0.0))).is_none());
        assert!(manifold((&triangle, &at(0.0, 0.0)), (&triangle, &at(1.9, 0.0))).is_some());
    }

    #[test]
    fn far_from_the_origin_a_contact_is_as_exact_as_near_it() {
        let (floor, _) = floor();
        let far = 100_000.0;
        let m = manifold((&boxed(0.5), &at(far, 0.4)), (&floor, &at(far, 0.0))).expect("overlap");
        for point in m.points() {
            assert!((point.separation + 0.1).abs() < 1e-2, "{point:?}");
        }
    }
}
