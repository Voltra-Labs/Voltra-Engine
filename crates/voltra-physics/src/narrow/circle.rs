//! Circle against circle: the one pair that is a subtraction.

use super::{Manifold, ManifoldPoint, Shape, EPSILON, FALLBACK_NORMAL};

pub(super) fn circle_circle(a: Shape<'_>, b: Shape<'_>) -> Option<Manifold> {
    let (ra, rb) = (a.0.world_radius(a.1), b.0.world_radius(b.1));
    let delta = a.1.translation - b.1.translation;
    let distance = delta.length();
    let penetration = ra + rb - distance;

    if penetration <= 0.0 {
        return None;
    }

    // Concentric: there is no direction between the centres to normalise.
    let normal = if distance > EPSILON {
        delta / distance
    } else {
        FALLBACK_NORMAL
    };

    // One point, and one feature: a circle has nowhere else to touch, so the
    // id is the constant every single-point manifold uses.
    Some(Manifold::new(
        normal,
        &[ManifoldPoint {
            point: b.1.translation + normal * rb,
            separation: -penetration,
            id: 0,
        }],
    ))
}

#[cfg(test)]
mod tests {
    use crate::narrow::manifold;
    use crate::narrow::tests::{at, circle};
    use voltra_render::glam::Vec2;

    #[test]
    fn separated_circles_do_not_touch() {
        assert!(manifold((&circle(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(5.0, 0.0))).is_none());
    }

    #[test]
    fn overlapping_circles_report_the_overlap_along_the_centre_line() {
        let m = manifold((&circle(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(1.5, 0.0)))
            .expect("they overlap");

        // The normal pushes `a` away from `b`, so it points in -x.
        assert!(
            (m.normal - Vec2::new(-1.0, 0.0)).length() < 1e-6,
            "{:?}",
            m.normal
        );
        assert!((m.points()[0].separation + 0.5).abs() < 1e-6, "{m:?}");
    }

    #[test]
    fn a_circle_pair_is_one_point_with_no_feature() {
        let m = manifold((&circle(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(1.5, 0.0)))
            .expect("they overlap");

        assert_eq!(m.points().len(), 1);
        assert_eq!(m.points()[0].id, 0);
    }

    #[test]
    fn circles_touching_exactly_do_not_report_a_contact() {
        // Zero penetration is not a collision. Reporting it hands the solver a
        // contact with nothing to resolve, every frame, for every pair of
        // bodies resting against each other.
        assert!(manifold((&circle(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(2.0, 0.0))).is_none());
    }

    #[test]
    fn concentric_circles_give_a_unit_normal_rather_than_nan() {
        let m = manifold((&circle(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(0.0, 0.0)))
            .expect("fully overlapping");

        assert!(m.normal.is_finite(), "{m:?}");
        assert!((m.normal.length() - 1.0).abs() < 1e-6, "{m:?}");
        let separation = m.points()[0].separation;
        assert!(separation.is_finite() && separation < 0.0, "{m:?}");
    }

    #[test]
    fn swapping_two_circles_negates_the_normal() {
        let one = manifold((&circle(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(1.5, 0.0)))
            .expect("overlap");
        let other = manifold((&circle(1.0), &at(1.5, 0.0)), (&circle(1.0), &at(0.0, 0.0)))
            .expect("overlap");

        assert!((one.normal + other.normal).length() < 1e-6);
    }
}
