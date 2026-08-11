//! Box against circle, and the mirrored order with it.

use voltra_render::glam::Vec2;

use super::{sign_away, Manifold, ManifoldPoint, Shape, EPSILON};

pub(super) fn box_circle(a: Shape<'_>, b: Shape<'_>) -> Option<Manifold> {
    let half = a.0.world_half_extents(a.1);
    let radius = b.0.world_radius(b.1);

    // Solved in the box's own frame, where it is axis-aligned again and the
    // clamp below is two comparisons. The answer is turned back at the end.
    let into_box = Vec2::from_angle(-a.1.rotation);
    let into_world = Vec2::from_angle(a.1.rotation);
    let centre = into_box.rotate(b.1.translation - a.1.translation);
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
        let normal = into_world.rotate(-offset / distance);
        return Some(Manifold::new(
            normal,
            &[ManifoldPoint {
                point: a.1.translation + into_world.rotate(closest),
                separation: -penetration,
                id: 0,
            }],
        ));
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

    Some(Manifold::new(
        into_world.rotate(normal),
        &[ManifoldPoint {
            point: b.1.translation,
            separation: -penetration,
            id: 0,
        }],
    ))
}

#[cfg(test)]
mod tests {
    use crate::narrow::manifold;
    use crate::narrow::tests::{at, boxed, circle};
    use std::f32::consts::FRAC_PI_2;
    use voltra_render::glam::Vec2;
    use voltra_scene::{Collider, Transform};

    #[test]
    fn a_circle_beside_a_box_pushes_out_of_the_nearest_face() {
        let m = manifold((&boxed(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(1.5, 0.0)))
            .expect("they overlap");

        assert!(
            (m.normal - Vec2::new(-1.0, 0.0)).length() < 1e-6,
            "{:?}",
            m.normal
        );
        assert!((m.points()[0].separation + 0.5).abs() < 1e-6, "{m:?}");
    }

    #[test]
    fn turning_the_box_is_what_decides_whether_it_touches() {
        // A tall box and a circle beside it: clear of the narrow side, well
        // inside the long one. A quarter turn swaps which is which, and an
        // axis-aligned test would answer the same both times.
        let tall = Collider::Box {
            half_extents: Vec2::new(0.5, 2.0),
        };
        let ball = circle(0.5);

        assert!(
            manifold((&tall, &Transform::default()), (&ball, &at(1.2, 0.0))).is_none(),
            "0.7 clear of the narrow side"
        );

        let turned = Transform::default().with_rotation(FRAC_PI_2);
        let m = manifold((&tall, &turned), (&ball, &at(1.2, 0.0))).expect("now the long side");

        assert!(m.points()[0].separation < 0.0, "{m:?}");
        assert!((m.normal.length() - 1.0).abs() < 1e-5, "{m:?}");
    }

    #[test]
    fn a_circle_at_a_boxs_centre_still_gives_a_finite_normal() {
        // The closest point on the box to the centre *is* the centre, so the
        // usual difference is zero and normalising it is NaN.
        let m = manifold((&boxed(2.0), &at(0.0, 0.0)), (&circle(0.5), &at(0.0, 0.0)))
            .expect("fully inside");

        assert!(m.normal.is_finite(), "{m:?}");
        assert!((m.normal.length() - 1.0).abs() < 1e-6, "{m:?}");
        assert!(m.points()[0].separation < 0.0, "{m:?}");
    }

    #[test]
    fn a_circle_inside_a_box_leaves_by_its_nearest_face() {
        // Nearer the right face than the top, so it goes out sideways.
        let m =
            manifold((&boxed(4.0), &at(0.0, 0.0)), (&circle(0.5), &at(3.5, 0.0))).expect("inside");

        assert!(m.normal.x < -0.9, "expected -x, got {:?}", m.normal);
    }

    #[test]
    fn a_box_beside_a_circle_mirrors_the_circle_beside_a_box() {
        // Argument order must not change the physics, only the normal's sign.
        let one =
            manifold((&boxed(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(1.5, 0.0))).expect("overlap");
        let other =
            manifold((&circle(1.0), &at(1.5, 0.0)), (&boxed(1.0), &at(0.0, 0.0))).expect("overlap");

        assert!(
            (one.normal + other.normal).length() < 1e-6,
            "{:?} {:?}",
            one.normal,
            other.normal
        );
        assert!(
            (one.points()[0].separation - other.points()[0].separation).abs() < 1e-6,
            "{one:?} {other:?}"
        );
    }
}
