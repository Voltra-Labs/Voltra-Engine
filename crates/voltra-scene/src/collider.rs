//! The shape an entity occupies, at the origin.
//!
//! Where it sits — and which way it faces — comes from the entity's
//! [`Transform`], so a collider is a size and nothing else. The box is
//! *oriented*: it turns with the sprite it covers. Only its world-space
//! bounds, which the broad phase compares, stay axis-aligned.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;

use crate::Transform;

/// The shape used for collision.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Collider {
    /// Box centred on the transform, turned by its rotation.
    ///
    /// Named `Box` rather than `Aabb` since 11b-3: it is only axis-aligned
    /// when the transform is, and a name that says otherwise is the kind of
    /// lie that outlives the code it came from.
    Box {
        half_extents: Vec2,
    },
    Circle {
        radius: f32,
    },
}

impl Collider {
    /// The world-space bounds, as `(min, max)`.
    ///
    /// Scale is applied, and its absolute value is taken: a mirrored sprite has
    /// a negative scale, and a box whose min exceeds its max overlaps nothing —
    /// the collider would silently stop working on exactly the sprites facing
    /// left.
    pub fn world_aabb(&self, transform: &Transform) -> (Vec2, Vec2) {
        let half = match self {
            Self::Box { .. } => {
                // The bound of a turned box: each axis takes both extents,
                // weighted by how far it has turned. The absolute values are
                // what make the bound hold in all four quadrants — the sign of
                // a sine only mirrors the corner it came from.
                let (sin, cos) = transform.rotation.sin_cos();
                let half = self.world_half_extents(transform);
                Vec2::new(
                    cos.abs() * half.x + sin.abs() * half.y,
                    sin.abs() * half.x + cos.abs() * half.y,
                )
            }
            // Clamped so a nonsense radius gives an empty box rather than an
            // inverted one. `is_degenerate` is what rejects it; this only makes
            // sure the bounds stay well-formed on the way there.
            Self::Circle { .. } => Vec2::splat(self.world_radius(transform).max(0.0)),
        };
        (transform.translation - half, transform.translation + half)
    }

    /// The four corners in world space, counter-clockwise from `(+hx, +hy)`.
    ///
    /// Four copies of the centre for a circle, which has no corners: callers
    /// that care about the difference match on the variant first, and the
    /// degenerate answer is a point rather than a shape of the wrong size.
    pub fn corners(&self, transform: &Transform) -> [Vec2; 4] {
        let half = self.world_half_extents(transform);
        let rotation = Vec2::from_angle(transform.rotation);
        [
            Vec2::new(half.x, half.y),
            Vec2::new(-half.x, half.y),
            Vec2::new(-half.x, -half.y),
            Vec2::new(half.x, -half.y),
        ]
        .map(|corner| transform.translation + rotation.rotate(corner))
    }

    /// The scaled half extents, for a box. `Vec2::ZERO` for a circle.
    pub fn world_half_extents(&self, transform: &Transform) -> Vec2 {
        match self {
            Self::Box { half_extents } => (*half_extents * transform.scale).abs(),
            Self::Circle { .. } => Vec2::ZERO,
        }
    }

    /// The scaled radius, for a circle. Zero for a box.
    ///
    /// A non-uniform scale takes the larger axis. A true ellipse is a different
    /// shape rather than a parameter of this one, and the larger axis keeps the
    /// collider covering the sprite instead of cutting into it.
    ///
    /// The *scale* is taken as its absolute value and the *radius* is not, and
    /// that asymmetry is the point: a negative scale is a mirrored sprite and
    /// means something, while a negative radius is nonsense a scene file
    /// happens to contain. Mirroring is honoured; nonsense stays negative so
    /// [`Self::is_degenerate`] can catch it instead of it quietly becoming a
    /// working collider of the wrong provenance.
    pub fn world_radius(&self, transform: &Transform) -> f32 {
        match self {
            Self::Circle { radius } => radius * transform.scale.abs().max_element(),
            Self::Box { .. } => 0.0,
        }
    }

    /// Whether this shape has any area once scaled.
    ///
    /// A scene file can contain a zero or negative radius, and an inverted
    /// shape reports contacts with a backwards normal.
    pub fn is_degenerate(&self, transform: &Transform) -> bool {
        match self {
            Self::Box { .. } => {
                let half = self.world_half_extents(transform);
                half.x <= 0.0 || half.y <= 0.0
            }
            Self::Circle { .. } => self.world_radius(transform) <= 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rotated_box_is_bounded_by_a_larger_aabb() {
        use std::f32::consts::{FRAC_PI_4, SQRT_2};

        let collider = Collider::Box {
            half_extents: Vec2::splat(1.0),
        };
        let transform = Transform::default().with_rotation(FRAC_PI_4);

        let (min, max) = collider.world_aabb(&transform);

        // A unit square turned 45° spans its diagonal, √2 either side.
        assert!((max.x - SQRT_2).abs() < 1e-5, "{max:?}");
        assert!((min.y + SQRT_2).abs() < 1e-5, "{min:?}");
    }

    #[test]
    fn the_corners_follow_the_rotation_counter_clockwise() {
        use std::f32::consts::FRAC_PI_2;

        let collider = Collider::Box {
            half_extents: Vec2::splat(1.0),
        };
        let corners = collider.corners(&Transform::default().with_rotation(FRAC_PI_2));

        // (+1,+1) turned a quarter turn is (-1,+1).
        assert!(
            (corners[0] - Vec2::new(-1.0, 1.0)).length() < 1e-5,
            "{corners:?}"
        );
        // Counter-clockwise, so the shoelace sum is positive. The clipping in
        // the narrow phase depends on this winding.
        let area: f32 = (0..4)
            .map(|i| {
                let (p, q) = (corners[i], corners[(i + 1) % 4]);
                p.x * q.y - q.x * p.y
            })
            .sum();
        assert!(area > 0.0, "{corners:?}");
    }

    #[test]
    fn the_corners_follow_the_translation_and_scale() {
        let collider = Collider::Box {
            half_extents: Vec2::splat(1.0),
        };
        let transform =
            Transform::from_translation(Vec2::new(5.0, 0.0)).with_scale(Vec2::splat(2.0));

        let corners = collider.corners(&transform);

        assert!(
            (corners[0] - Vec2::new(7.0, 2.0)).length() < 1e-5,
            "{corners:?}"
        );
        assert!(
            (corners[2] - Vec2::new(3.0, -2.0)).length() < 1e-5,
            "{corners:?}"
        );
    }

    #[test]
    fn a_circle_has_no_corners_but_still_answers() {
        let collider = Collider::Circle { radius: 2.0 };
        let transform = Transform::from_translation(Vec2::new(1.0, 1.0));

        assert_eq!(collider.corners(&transform), [transform.translation; 4]);
    }

    #[test]
    fn a_box_spans_twice_its_half_extents() {
        let collider = Collider::Box {
            half_extents: Vec2::new(2.0, 3.0),
        };
        let (min, max) = collider.world_aabb(&Transform::default());

        assert_eq!(min, Vec2::new(-2.0, -3.0));
        assert_eq!(max, Vec2::new(2.0, 3.0));
    }

    #[test]
    fn a_box_follows_the_transform_translation() {
        let collider = Collider::Box {
            half_extents: Vec2::splat(1.0),
        };
        let transform = Transform::from_translation(Vec2::new(10.0, -4.0));
        let (min, max) = collider.world_aabb(&transform);

        assert_eq!(min, Vec2::new(9.0, -5.0));
        assert_eq!(max, Vec2::new(11.0, -3.0));
    }

    #[test]
    fn a_collider_is_scaled_by_its_transform() {
        // A scaled sprite with an unscaled collider is a bug that looks like a
        // physics bug: the outline and the picture disagree and neither is
        // obviously the wrong one.
        let collider = Collider::Box {
            half_extents: Vec2::splat(1.0),
        };
        let transform = Transform::default().with_scale(Vec2::new(3.0, 5.0));
        let (min, max) = collider.world_aabb(&transform);

        assert_eq!(min, Vec2::new(-3.0, -5.0));
        assert_eq!(max, Vec2::new(3.0, 5.0));
    }

    #[test]
    fn a_negative_scale_still_gives_a_min_below_its_max() {
        // A mirrored sprite is a negative scale, and an AABB whose min exceeds
        // its max overlaps nothing — the collider would silently stop working.
        let collider = Collider::Box {
            half_extents: Vec2::splat(2.0),
        };
        let transform = Transform::default().with_scale(Vec2::new(-1.0, 1.0));
        let (min, max) = collider.world_aabb(&transform);

        assert!(min.x <= max.x && min.y <= max.y, "got {min:?}..{max:?}");
    }

    #[test]
    fn a_circle_takes_the_larger_axis_of_a_non_uniform_scale() {
        // A true ellipse is a different shape, not a parameter. Taking the
        // larger axis keeps the collider covering the sprite rather than
        // cutting into it.
        let collider = Collider::Circle { radius: 1.0 };
        let transform = Transform::default().with_scale(Vec2::new(2.0, 5.0));

        assert_eq!(collider.world_radius(&transform), 5.0);
    }

    #[test]
    fn a_circles_aabb_is_its_bounding_square() {
        let collider = Collider::Circle { radius: 2.0 };
        let (min, max) = collider.world_aabb(&Transform::default());

        assert_eq!(min, Vec2::splat(-2.0));
        assert_eq!(max, Vec2::splat(2.0));
    }

    #[test]
    fn a_zero_or_negative_shape_is_degenerate() {
        let t = Transform::default();

        assert!(Collider::Circle { radius: 0.0 }.is_degenerate(&t));
        assert!(Collider::Circle { radius: -1.0 }.is_degenerate(&t));
        assert!(Collider::Box {
            half_extents: Vec2::ZERO
        }
        .is_degenerate(&t));
        assert!(!Collider::Circle { radius: 1.0 }.is_degenerate(&t));
    }

    #[test]
    fn a_shape_scaled_to_nothing_is_degenerate() {
        let flat = Transform::default().with_scale(Vec2::new(1.0, 0.0));

        assert!(Collider::Box {
            half_extents: Vec2::splat(1.0)
        }
        .is_degenerate(&flat));
    }

    #[test]
    fn a_collider_round_trips_through_ron() {
        for collider in [
            Collider::Box {
                half_extents: Vec2::new(1.0, 2.0),
            },
            Collider::Circle { radius: 3.0 },
        ] {
            let text = ron::to_string(&collider).expect("serialise");
            let back: Collider = ron::from_str(&text).expect("deserialise");
            assert_eq!(back, collider);
        }
    }
}
