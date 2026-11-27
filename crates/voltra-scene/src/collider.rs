//! The shape an entity occupies, at the origin.
//!
//! Where it sits — and which way it faces — comes from the entity's
//! [`Transform2D`], so a collider is a size and nothing else. Every shape is
//! *oriented*: it turns with the sprite it covers. Only its world-space
//! bounds, which the broad phase compares, stay axis-aligned.
//!
//! [`Collider2D::world_shape`] places any of them as one thing — world points
//! grown by a radius — which is what the physics collides.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;

use crate::Transform2D;

mod polygon;
mod world_shape;

pub use polygon::{ConvexPolygon, PolygonError};
pub use world_shape::WorldShape;

/// The shape used for collision.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Collider2D {
    /// Box centred on the transform, turned by its rotation.
    ///
    /// Named `Box` rather than `Aabb` since 11b-3: it is only axis-aligned
    /// when the transform is, and a name that says otherwise is the kind of
    /// lie that outlives the code it came from.
    Box {
        /// Half the width and height, before the transform's scale.
        half_extents: Vec2,
    },
    /// A circle around the transform's origin.
    Circle {
        /// Before the transform's scale, which applies its larger axis.
        radius: f32,
    },
    /// A rectangle with half-circle ends, upright along the entity's y:
    /// what a character stands in, since a rounded bottom does not catch on
    /// the seam between two floor tiles.
    ///
    /// `half_height` is half the whole height, caps included — the same
    /// measure as a box's `half_extents.y`, so a box swapped for a capsule
    /// keeps its feet where they were. A capsule is never shorter than it is
    /// wide: below `radius` it is a circle. The radius scales with the
    /// transform's x, the height with its y.
    Capsule {
        /// Half the whole height, caps included, before the transform's y scale.
        half_height: f32,
        /// Of the half-circle ends, before the transform's x scale.
        radius: f32,
    },
    /// A convex polygon of up to eight points, around the transform's origin,
    /// optionally rounded by `radius`.
    ///
    /// The body's mass sits at the transform's origin whatever the points, so
    /// a dynamic polygon is drawn around it; a static one can sit anywhere.
    Polygon {
        /// The corners, convex, around the transform's origin.
        points: ConvexPolygon,
        /// How far the surface stands off the points: zero is sharp corners.
        /// Scales with the larger axis of the transform, as a circle does.
        #[serde(default)]
        radius: f32,
    },
}

impl Default for Collider2D {
    /// A box the size of a default sprite, which is what the editor gives a
    /// spawned body: the collider covers what is drawn.
    fn default() -> Self {
        Self::Box {
            half_extents: Vec2::splat(crate::Sprite2D::HALF_EXTENT),
        }
    }
}

impl Collider2D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Collider2D";

    /// The shape's name, as the inspector and the logs show it.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Box { .. } => "Box",
            Self::Circle { .. } => "Circle",
            Self::Capsule { .. } => "Capsule",
            Self::Polygon { .. } => "Polygon",
        }
    }

    /// The world-space bounds, as `(min, max)`.
    ///
    /// Scale is applied, and its absolute value is taken: a mirrored sprite has
    /// a negative scale, and a box whose min exceeds its max overlaps nothing —
    /// the collider would silently stop working on exactly the sprites facing
    /// left.
    pub fn world_aabb(&self, transform: &Transform2D) -> (Vec2, Vec2) {
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
            Self::Capsule { .. } | Self::Polygon { .. } => {
                return self.world_shape(transform).bounds();
            }
        };
        (transform.translation - half, transform.translation + half)
    }

    /// The shape in world space: its hull's points, grown by its radius.
    ///
    /// A box is its four corners; a circle its centre; a capsule the centres
    /// of its two caps — or one, once it is no taller than wide; a polygon its
    /// points, scaled, turned and placed, still counter-clockwise under a
    /// mirroring scale.
    pub fn world_shape(&self, transform: &Transform2D) -> WorldShape {
        match self {
            Self::Box { .. } => WorldShape::new(&self.corners(transform), 0.0),
            Self::Circle { .. } => {
                WorldShape::new(&[transform.translation], self.world_radius(transform))
            }
            Self::Capsule { .. } => {
                let radius = self.world_radius(transform);
                let reach = self.capsule_reach(transform);
                if reach <= radius.abs() * 1e-4 {
                    return WorldShape::new(&[transform.translation], radius);
                }
                let up = Vec2::from_angle(transform.rotation).rotate(Vec2::Y) * reach;
                WorldShape::new(
                    &[transform.translation - up, transform.translation + up],
                    radius,
                )
            }
            Self::Polygon { points, .. } => {
                let rotation = Vec2::from_angle(transform.rotation);
                let mut placed = [Vec2::ZERO; ConvexPolygon::MAX_POINTS];
                let points = points.points();
                for (out, point) in placed.iter_mut().zip(points) {
                    *out = transform.translation + rotation.rotate(*point * transform.scale);
                }
                let placed = &mut placed[..points.len()];
                // A mirror turns the winding round; the narrow phase's
                // normals depend on it running counter-clockwise.
                if transform.scale.x * transform.scale.y < 0.0 {
                    placed.reverse();
                }
                WorldShape::new(placed, self.world_radius(transform))
            }
        }
    }

    /// How far a capsule's cap centres sit from its middle, in world units.
    fn capsule_reach(&self, transform: &Transform2D) -> f32 {
        match self {
            Self::Capsule { half_height, .. } => {
                (half_height * transform.scale.y.abs() - self.world_radius(transform)).max(0.0)
            }
            _ => 0.0,
        }
    }

    /// The surface in world space as a closed polyline, counter-clockwise,
    /// with round parts cut so that a full turn takes `sides` segments.
    pub fn outline(&self, transform: &Transform2D, sides: usize) -> Vec<Vec2> {
        self.world_shape(transform).outline(sides)
    }

    /// The four corners in world space, counter-clockwise from `(+hx, +hy)`.
    ///
    /// Four copies of the centre for any other shape, which has no such
    /// corners: callers that care about the difference match on the variant
    /// first, and the degenerate answer is a point rather than a shape of the
    /// wrong size.
    pub fn corners(&self, transform: &Transform2D) -> [Vec2; 4] {
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

    /// The scaled half extents, for a box. `Vec2::ZERO` for any other shape.
    pub fn world_half_extents(&self, transform: &Transform2D) -> Vec2 {
        match self {
            Self::Box { half_extents } => (*half_extents * transform.scale).abs(),
            _ => Vec2::ZERO,
        }
    }

    /// The scaled radius: a circle's, a capsule's caps', a polygon's
    /// rounding. Zero for a box.
    ///
    /// A non-uniform scale takes the larger axis for a circle and a polygon.
    /// A true ellipse is a different shape rather than a parameter of this
    /// one, and the larger axis keeps the collider covering the sprite instead
    /// of cutting into it. A capsule's radius takes the x axis, across it.
    ///
    /// The *scale* is taken as its absolute value and the *radius* is not, and
    /// that asymmetry is the point: a negative scale is a mirrored sprite and
    /// means something, while a negative radius is nonsense a scene file
    /// happens to contain. Mirroring is honoured; nonsense stays negative so
    /// [`Self::is_degenerate`] can catch it instead of it quietly becoming a
    /// working collider of the wrong provenance.
    pub fn world_radius(&self, transform: &Transform2D) -> f32 {
        match self {
            Self::Circle { radius } | Self::Polygon { radius, .. } => {
                radius * transform.scale.abs().max_element()
            }
            Self::Capsule { radius, .. } => radius * transform.scale.x.abs(),
            Self::Box { .. } => 0.0,
        }
    }

    /// Whether this shape has any area once scaled.
    ///
    /// A scene file can contain a zero or negative radius, and an inverted
    /// shape reports contacts with a backwards normal. A polygon squashed
    /// flat by its scale has no area whatever its rounding: its points no
    /// longer have an inside to push out of. Anything that is not a number
    /// is degenerate too.
    pub fn is_degenerate(&self, transform: &Transform2D) -> bool {
        match self {
            Self::Box { .. } => {
                let half = self.world_half_extents(transform);
                !(half.x > 0.0 && half.y > 0.0)
            }
            Self::Circle { .. } | Self::Capsule { .. } => {
                let radius = self.world_radius(transform);
                !(radius > 0.0 && self.capsule_reach(transform).is_finite())
            }
            Self::Polygon { points, radius } => {
                let squash = (transform.scale.x * transform.scale.y).abs();
                !(points.area() * squash > 0.0 && *radius >= 0.0 && radius.is_finite())
            }
        }
    }
}

#[cfg(test)]
mod tests;
