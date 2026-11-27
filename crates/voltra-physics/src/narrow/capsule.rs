//! Capsule against capsule: the closest points of two segments, clipped to
//! two points where they lie alongside — Box2D v3's `b2CollideCapsules`.

use voltra_render::glam::Vec2;

use super::{round::point_point, Manifold, ManifoldPoint, EPSILON};
use crate::geometry::{segment_distance, Hull};

pub(super) fn capsule_capsule(a: &Hull, b: &Hull) -> Option<Manifold> {
    let (p1, q1) = (a.vertices()[0], a.vertices()[1]);
    let (p2, q2) = (b.vertices()[0], b.vertices()[1]);
    let radius = a.radius + b.radius;
    let closest = segment_distance(p1, q1, p2, q2);
    if closest.distance_squared >= radius * radius {
        return None;
    }

    let (length1, length2) = ((q1 - p1).length(), (q2 - p2).length());
    let (u1, u2) = ((q1 - p1) / length1, (q2 - p2) / length2);
    // B's ends along A, and A's along B: wholly past one end is a cap
    // against the other capsule, and one point.
    let (fp2, fq2) = ((p2 - p1).dot(u1), (q2 - p1).dot(u1));
    let outside_a = (fp2 <= 0.0 && fq2 <= 0.0) || (fp2 >= length1 && fq2 >= length1);
    let (fp1, fq1) = ((p1 - p2).dot(u2), (q1 - p2).dot(u2));
    let outside_b = (fp1 <= 0.0 && fq1 <= 0.0) || (fp1 >= length2 && fq1 >= length2);

    if !outside_a && !outside_b {
        if let Some(manifold) = alongside(a, b, closest.closest2 - closest.closest1, u1, length1) {
            return Some(manifold);
        }
    }
    point_point(closest.closest1, a.radius, closest.closest2, b.radius).map(|m| {
        // Segments that cross have no line between their closest points.
        if closest.distance_squared > EPSILON * EPSILON {
            m
        } else {
            let normal = side(a, b, u1);
            Manifold::new(-normal, m.points())
        }
    })
}

/// B's segment clipped to A's length, measured from A's side facing B.
fn alongside(a: &Hull, b: &Hull, between: Vec2, u1: Vec2, length1: f32) -> Option<Manifold> {
    let (p1, p2, q2) = (a.vertices()[0], b.vertices()[0], b.vertices()[1]);
    let (fp2, fq2) = ((p2 - p1).dot(u1), (q2 - p1).dot(u1));
    let lower = if fp2 < 0.0 && fq2 - fp2 > f32::EPSILON {
        p2.lerp(q2, -fp2 / (fq2 - fp2))
    } else if fq2 < 0.0 && fp2 - fq2 > f32::EPSILON {
        q2.lerp(p2, -fq2 / (fp2 - fq2))
    } else if fp2 < fq2 {
        p2
    } else {
        q2
    };
    let upper = if fp2 > length1 && fp2 - fq2 > f32::EPSILON {
        p2.lerp(q2, (fp2 - length1) / (fp2 - fq2))
    } else if fq2 > length1 && fq2 - fp2 > f32::EPSILON {
        q2.lerp(p2, (fq2 - length1) / (fq2 - fp2))
    } else if fp2 < fq2 {
        q2
    } else {
        p2
    };

    let normal = if between.length_squared() > EPSILON * EPSILON {
        let perp = u1.perp();
        if perp.dot(between) < 0.0 {
            -perp
        } else {
            perp
        }
    } else {
        side(a, b, u1)
    };
    let radius = a.radius + b.radius;
    let mut points = [ManifoldPoint {
        point: Vec2::ZERO,
        separation: 0.0,
        id: 0,
    }; 2];
    let mut count = 0;
    for (id, end) in [lower, upper].into_iter().enumerate() {
        let above = normal.dot(end - p1);
        let separation = above - radius;
        if separation < 0.0 {
            points[count] = ManifoldPoint {
                point: end + normal * (0.5 * (a.radius - b.radius - above)),
                separation,
                id: id as u16,
            };
            count += 1;
        }
    }
    (count > 0).then(|| Manifold::new(-normal, &points[..count]))
}

/// Which side of A's segment B is mostly on, as A's normal towards it.
fn side(a: &Hull, b: &Hull, u1: Vec2) -> Vec2 {
    let middle_b = (b.vertices()[0] + b.vertices()[1]) * 0.5;
    let perp = u1.perp();
    if perp.dot(middle_b - a.vertices()[0]) < 0.0 {
        -perp
    } else {
        perp
    }
}

#[cfg(test)]
mod tests {
    use crate::narrow::manifold;
    use crate::narrow::tests::at;
    use std::f32::consts::FRAC_PI_2;
    use voltra_render::glam::Vec2;
    use voltra_scene::{Collider2D, Transform2D};

    fn capsule() -> Collider2D {
        Collider2D::Capsule {
            half_height: 1.0,
            radius: 0.25,
        }
    }

    fn lying(x: f32, y: f32) -> Transform2D {
        at(x, y).with_rotation(FRAC_PI_2)
    }

    #[test]
    fn two_capsules_lying_one_on_the_other_touch_twice() {
        let m = manifold(
            (&capsule(), &lying(0.3, 0.4)),
            (&capsule(), &lying(0.0, 0.0)),
        )
        .expect("overlap");

        assert_eq!(m.points().len(), 2, "{m:?}");
        assert!((m.normal - Vec2::Y).length() < 1e-5, "{m:?}");
        for point in m.points() {
            assert!((point.separation + 0.1).abs() < 1e-5, "{point:?}");
            assert!((point.point.y - 0.2).abs() < 1e-5, "{point:?}");
        }
    }

    #[test]
    fn capsules_end_to_end_touch_once_along_their_line() {
        let m =
            manifold((&capsule(), &at(0.0, 1.9)), (&capsule(), &at(0.0, 0.0))).expect("overlap");

        assert_eq!(m.points().len(), 1, "{m:?}");
        assert!((m.normal - Vec2::Y).length() < 1e-5, "{m:?}");
        assert!((m.points()[0].separation + 0.1).abs() < 1e-5, "{m:?}");
    }

    #[test]
    fn crossing_capsules_get_a_unit_normal() {
        let m = manifold((&capsule(), &at(0.0, 0.0)), (&capsule(), &lying(0.0, 0.1)))
            .expect("crossing");
        assert!(
            m.normal.is_finite() && (m.normal.length() - 1.0).abs() < 1e-5,
            "{m:?}"
        );
        assert!(m.deepest_separation() < -0.25, "{m:?}");
    }

    #[test]
    fn capsules_apart_do_not_touch() {
        assert!(manifold((&capsule(), &at(0.0, 0.0)), (&capsule(), &at(0.6, 0.0))).is_none());
        assert!(manifold((&capsule(), &at(0.0, 0.0)), (&capsule(), &at(0.0, 2.1))).is_none());
    }

    #[test]
    fn swapping_two_capsules_negates_the_normal() {
        let one = manifold((&capsule(), &at(0.0, 0.0)), (&capsule(), &at(0.4, 0.3))).expect("a");
        let other = manifold((&capsule(), &at(0.4, 0.3)), (&capsule(), &at(0.0, 0.0))).expect("b");
        assert!(
            (one.normal + other.normal).length() < 1e-5,
            "{one:?} {other:?}"
        );
    }
}
