//! The shape an entity occupies, at the origin.
//!
//! Where it sits comes from the entity's [`Transform`], so a collider is a size
//! and nothing else. That is also why [`Collider::Aabb`] does not rotate: it is
//! axis-aligned in *world* space, not in the entity's. A rotated sprite keeps
//! an upright box, which is a real limitation and the reason oriented boxes and
//! polygons belong to a later stage rather than being pretended at here.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;

use crate::Transform;

/// The shape used for collision.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Collider {
    /// Axis-aligned box, centred on the transform.
    Aabb {
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
            Self::Aabb { half_extents } => (*half_extents * transform.scale).abs(),
            // Clamped so a nonsense radius gives an empty box rather than an
            // inverted one. `is_degenerate` is what rejects it; this only makes
            // sure the bounds stay well-formed on the way there.
            Self::Circle { .. } => Vec2::splat(self.world_radius(transform).max(0.0)),
        };
        (transform.translation - half, transform.translation + half)
    }

    /// The scaled half extents, for a box. `Vec2::ZERO` for a circle.
    pub fn world_half_extents(&self, transform: &Transform) -> Vec2 {
        match self {
            Self::Aabb { half_extents } => (*half_extents * transform.scale).abs(),
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
            Self::Aabb { .. } => 0.0,
        }
    }

    /// Whether this shape has any area once scaled.
    ///
    /// A scene file can contain a zero or negative radius, and an inverted
    /// shape reports contacts with a backwards normal.
    pub fn is_degenerate(&self, transform: &Transform) -> bool {
        match self {
            Self::Aabb { .. } => {
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
    fn an_aabb_spans_twice_its_half_extents() {
        let collider = Collider::Aabb {
            half_extents: Vec2::new(2.0, 3.0),
        };
        let (min, max) = collider.world_aabb(&Transform::default());

        assert_eq!(min, Vec2::new(-2.0, -3.0));
        assert_eq!(max, Vec2::new(2.0, 3.0));
    }

    #[test]
    fn an_aabb_follows_the_transform_translation() {
        let collider = Collider::Aabb {
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
        let collider = Collider::Aabb {
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
        let collider = Collider::Aabb {
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
        assert!(Collider::Aabb {
            half_extents: Vec2::ZERO
        }
        .is_degenerate(&t));
        assert!(!Collider::Circle { radius: 1.0 }.is_degenerate(&t));
    }

    #[test]
    fn a_shape_scaled_to_nothing_is_degenerate() {
        let flat = Transform::default().with_scale(Vec2::new(1.0, 0.0));

        assert!(Collider::Aabb {
            half_extents: Vec2::splat(1.0)
        }
        .is_degenerate(&flat));
    }

    #[test]
    fn a_collider_round_trips_through_ron() {
        for collider in [
            Collider::Aabb {
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
