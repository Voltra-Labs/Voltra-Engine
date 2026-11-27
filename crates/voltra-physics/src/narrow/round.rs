//! Pairs where one side is a circle — a single corner grown by its radius —
//! against a circle, a capsule's segment, or a polygon's face or corner.

use voltra_render::glam::Vec2;

use super::{Manifold, ManifoldPoint, EPSILON, FALLBACK_NORMAL};
use crate::geometry::{nearest_on_segment, Hull};

/// A shape of two or more corners against a circle at `centre`.
pub(super) fn hull_circle(a: &Hull, centre: Vec2, radius_b: f32) -> Option<Manifold> {
    if a.len() == 2 {
        return segment_circle(a, centre, radius_b);
    }
    let radius = a.radius + radius_b;
    let (vertices, normals) = (a.vertices(), a.normals());

    // The face the centre is furthest out of.
    let (face, separation) = normals
        .iter()
        .zip(vertices)
        .map(|(n, v)| n.dot(centre - *v))
        .enumerate()
        .fold((0, f32::NEG_INFINITY), |best, (i, s)| {
            if s > best.1 {
                (i, s)
            } else {
                best
            }
        });
    if separation >= radius {
        return None;
    }
    let (v1, v2) = (vertices[face], vertices[a.next(face)]);
    let u1 = (centre - v1).dot(v2 - v1);
    let u2 = (centre - v2).dot(v1 - v2);
    // Outside the face's ends, and safely out of the core: a corner's circle.
    let corner = if u1 < 0.0 && separation > f32::EPSILON {
        Some(v1)
    } else if u2 < 0.0 && separation > f32::EPSILON {
        Some(v2)
    } else {
        None
    };
    if let Some(corner) = corner {
        return point_point(corner, a.radius, centre, radius_b);
    }
    // Between the face's ends, maybe inside the core.
    let normal = normals[face];
    let on_face = centre - normal * normal.dot(centre - v1);
    let surface_a = on_face + normal * a.radius;
    let surface_b = centre - normal * radius_b;
    Some(Manifold::new(
        -normal,
        &[ManifoldPoint {
            point: surface_a.lerp(surface_b, 0.5),
            separation: separation - radius,
            id: 0,
        }],
    ))
}

/// A capsule's segment against a circle: the nearest point of the segment
/// is a circle of the capsule's radius.
fn segment_circle(a: &Hull, centre: Vec2, radius_b: f32) -> Option<Manifold> {
    let v = a.vertices();
    let nearest = nearest_on_segment(v[0], v[1], centre);
    if (centre - nearest).length_squared() > EPSILON * EPSILON {
        return point_point(nearest, a.radius, centre, radius_b);
    }
    // On the segment itself: out through the side, either side.
    let normal = a.normals()[0];
    let separation = -(a.radius + radius_b);
    Some(Manifold::new(
        -normal,
        &[ManifoldPoint {
            point: centre + normal * (0.5 * (a.radius - radius_b)),
            separation,
            id: 0,
        }],
    ))
}

/// Two circles, as corners with radii: the normal pushes `a` away from `b`.
pub(super) fn point_point(a: Vec2, radius_a: f32, b: Vec2, radius_b: f32) -> Option<Manifold> {
    let delta = a - b;
    let distance = delta.length();
    let separation = distance - (radius_a + radius_b);
    if separation >= 0.0 {
        return None;
    }
    let normal = if distance > EPSILON {
        delta / distance
    } else {
        FALLBACK_NORMAL
    };
    let surface_a = a - normal * radius_a;
    let surface_b = b + normal * radius_b;
    Some(Manifold::new(
        normal,
        &[ManifoldPoint {
            point: surface_a.lerp(surface_b, 0.5),
            separation,
            id: 0,
        }],
    ))
}

#[cfg(test)]
mod tests {
    use crate::narrow::manifold;
    use crate::narrow::tests::{at, circle};
    use voltra_render::glam::Vec2;
    use voltra_scene::{Collider2D, ConvexPolygon};

    fn triangle() -> Collider2D {
        Collider2D::Polygon {
            points: ConvexPolygon::new(&[
                Vec2::new(-1.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(0.0, 1.0),
            ])
            .expect("a triangle"),
            radius: 0.0,
        }
    }

    fn capsule() -> Collider2D {
        Collider2D::Capsule {
            half_height: 1.0,
            radius: 0.5,
        }
    }

    #[test]
    fn a_circle_on_a_polygons_face_is_pushed_along_its_normal() {
        let m = manifold((&circle(0.5), &at(0.0, -0.4)), (&triangle(), &at(0.0, 0.0)))
            .expect("overlap");

        assert!((m.normal - Vec2::NEG_Y).length() < 1e-5, "{m:?}");
        assert!((m.points()[0].separation + 0.1).abs() < 1e-5, "{m:?}");
    }

    #[test]
    fn a_circle_past_a_polygons_corner_is_pushed_from_the_corner() {
        let m = manifold((&circle(0.5), &at(1.3, -0.3)), (&triangle(), &at(0.0, 0.0)))
            .expect("overlap");
        let away = Vec2::new(0.3, -0.3).normalize();

        assert!((m.normal - away).length() < 1e-5, "{m:?}");
        assert!(
            (m.points()[0].separation - (0.3 * 2f32.sqrt() - 0.5)).abs() < 1e-5,
            "{m:?}"
        );
    }

    #[test]
    fn a_circle_inside_a_polygon_leaves_by_the_nearest_face() {
        let m =
            manifold((&circle(0.1), &at(0.0, 0.1)), (&triangle(), &at(0.0, 0.0))).expect("inside");
        assert!(
            m.normal.is_finite() && (m.normal - Vec2::NEG_Y).length() < 1e-5,
            "{m:?}"
        );
        assert!((m.points()[0].separation + 0.2).abs() < 1e-5, "{m:?}");
    }

    #[test]
    fn a_circle_beside_a_capsule_and_on_its_cap() {
        let side =
            manifold((&capsule(), &at(0.0, 0.0)), (&circle(0.5), &at(0.9, 0.2))).expect("beside");
        assert!((side.normal - Vec2::NEG_X).length() < 1e-5, "{side:?}");
        assert!((side.points()[0].separation + 0.1).abs() < 1e-5, "{side:?}");

        let top = manifold((&capsule(), &at(0.0, 0.0)), (&circle(0.5), &at(0.0, 1.4)))
            .expect("on the cap");
        assert!((top.normal - Vec2::NEG_Y).length() < 1e-5, "{top:?}");
        assert!((top.points()[0].separation + 0.1).abs() < 1e-5, "{top:?}");
    }

    #[test]
    fn a_circle_centred_on_a_capsules_segment_still_gets_a_normal() {
        let m =
            manifold((&capsule(), &at(0.0, 0.0)), (&circle(0.5), &at(0.0, 0.1))).expect("inside");
        assert!(
            m.normal.is_finite() && (m.normal.length() - 1.0).abs() < 1e-5,
            "{m:?}"
        );
        assert!((m.points()[0].separation + 1.0).abs() < 1e-5, "{m:?}");
    }

    #[test]
    fn a_short_capsule_collides_as_the_circle_it_is() {
        let short = Collider2D::Capsule {
            half_height: 0.5,
            radius: 0.5,
        };
        let m = manifold((&short, &at(0.0, 0.0)), (&circle(0.5), &at(0.8, 0.0))).expect("overlap");
        assert!((m.points()[0].separation + 0.2).abs() < 1e-5, "{m:?}");
    }
}
