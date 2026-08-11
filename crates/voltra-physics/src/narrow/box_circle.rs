//! Box against circle, and the mirrored order with it.

use voltra_render::glam::Vec2;

use super::{sign_away, Shape, EPSILON};

pub(super) fn aabb_circle(a: Shape<'_>, b: Shape<'_>) -> Option<(Vec2, f32, Vec2)> {
    let half = a.0.world_half_extents(a.1);
    let radius = b.0.world_radius(b.1);
    let centre = b.1.translation - a.1.translation;
    let closest = centre.clamp(-half, half);
    let offset = centre - closest;

    if offset.length_squared() > EPSILON * EPSILON {
        // Outside the box: an ordinary circle-versus-point test.
        let distance = offset.length();
        let penetration = radius - distance;
        if penetration <= 0.0 {
            return None;
        }
        // `offset` points from the box towards the circle, so away from `b`
        // is its negation.
        let normal = -offset / distance;
        return Some((normal, penetration, a.1.translation + closest));
    }

    // The centre is inside the box, so the closest point *is* the centre and
    // the difference above is zero — normalising it would be NaN. The nearest
    // face is the answer instead.
    let to_face = half - centre.abs();
    let (normal, penetration) = if to_face.x < to_face.y {
        (Vec2::new(-sign_away(centre.x), 0.0), to_face.x + radius)
    } else {
        (Vec2::new(0.0, -sign_away(centre.y)), to_face.y + radius)
    };

    Some((normal, penetration, b.1.translation))
}

#[cfg(test)]
mod tests {
    use crate::narrow::contact;
    use crate::narrow::tests::{at, boxed, circle};
    use voltra_render::glam::Vec2;

    #[test]
    fn a_circle_beside_a_box_pushes_out_of_the_nearest_face() {
        let (normal, penetration, _) =
            contact((&boxed(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(1.5, 0.0)))
                .expect("they overlap");

        assert!(
            (normal - Vec2::new(-1.0, 0.0)).length() < 1e-6,
            "{normal:?}"
        );
        assert!((penetration - 0.5).abs() < 1e-6, "{penetration}");
    }

    #[test]
    fn a_circle_at_a_boxs_centre_still_gives_a_finite_normal() {
        // The closest point on the box to the centre *is* the centre, so the
        // usual difference is zero and normalising it is NaN.
        let (normal, penetration, _) =
            contact((&boxed(2.0), &at(0.0, 0.0)), (&circle(0.5), &at(0.0, 0.0)))
                .expect("fully inside");

        assert!(normal.is_finite(), "{normal:?}");
        assert!((normal.length() - 1.0).abs() < 1e-6, "{normal:?}");
        assert!(penetration > 0.0, "{penetration}");
    }

    #[test]
    fn a_circle_inside_a_box_leaves_by_its_nearest_face() {
        // Nearer the right face than the top, so it goes out sideways.
        let (normal, _, _) =
            contact((&boxed(4.0), &at(0.0, 0.0)), (&circle(0.5), &at(3.5, 0.0))).expect("inside");

        assert!(normal.x < -0.9, "expected -x, got {normal:?}");
    }

    #[test]
    fn a_box_beside_a_circle_mirrors_the_circle_beside_a_box() {
        // Argument order must not change the physics, only the normal's sign.
        let one =
            contact((&boxed(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(1.5, 0.0))).expect("overlap");
        let other =
            contact((&circle(1.0), &at(1.5, 0.0)), (&boxed(1.0), &at(0.0, 0.0))).expect("overlap");

        assert!(
            (one.0 + other.0).length() < 1e-6,
            "{:?} {:?}",
            one.0,
            other.0
        );
        assert!((one.1 - other.1).abs() < 1e-6);
    }
}
