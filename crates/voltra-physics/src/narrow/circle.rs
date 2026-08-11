//! Circle against circle: the one pair that is a subtraction.

use voltra_render::glam::Vec2;

use super::{Shape, EPSILON, FALLBACK_NORMAL};

pub(super) fn circle_circle(a: Shape<'_>, b: Shape<'_>) -> Option<(Vec2, f32, Vec2)> {
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

    Some((normal, penetration, b.1.translation + normal * rb))
}

#[cfg(test)]
mod tests {
    use crate::narrow::contact;
    use crate::narrow::tests::{at, circle};
    use voltra_render::glam::Vec2;

    #[test]
    fn separated_circles_do_not_touch() {
        assert!(contact((&circle(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(5.0, 0.0))).is_none());
    }

    #[test]
    fn overlapping_circles_report_the_overlap_along_the_centre_line() {
        let (normal, penetration, _) =
            contact((&circle(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(1.5, 0.0)))
                .expect("they overlap");

        // The normal pushes `a` away from `b`, so it points in -x.
        assert!(
            (normal - Vec2::new(-1.0, 0.0)).length() < 1e-6,
            "{normal:?}"
        );
        assert!((penetration - 0.5).abs() < 1e-6, "{penetration}");
    }

    #[test]
    fn circles_touching_exactly_do_not_report_a_contact() {
        // Zero penetration is not a collision. Reporting it hands the solver a
        // contact with nothing to resolve, every frame, for every pair of
        // bodies resting against each other.
        assert!(contact((&circle(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(2.0, 0.0))).is_none());
    }

    #[test]
    fn concentric_circles_give_a_unit_normal_rather_than_nan() {
        let (normal, penetration, _) =
            contact((&circle(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(0.0, 0.0)))
                .expect("fully overlapping");

        assert!(normal.is_finite(), "{normal:?}");
        assert!((normal.length() - 1.0).abs() < 1e-6, "{normal:?}");
        assert!(penetration.is_finite() && penetration > 0.0);
    }

    #[test]
    fn swapping_two_circles_negates_the_normal() {
        let one =
            contact((&circle(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(1.5, 0.0))).expect("overlap");
        let other =
            contact((&circle(1.0), &at(1.5, 0.0)), (&circle(1.0), &at(0.0, 0.0))).expect("overlap");

        assert!((one.0 + other.0).length() < 1e-6);
    }
}
