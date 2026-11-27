//! Where round an emitter its particles are born.
//!
//! Unity's Shape module and Godot's emission shapes, cut to what 2D needs:
//! a point, a disc, a ring, a box, a line, and the outline of the entity's
//! own collider — sparks off a hull, embers round a ring of fire. The
//! direction and spread still say which way they leave; the shape says only
//! where from.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;
use voltra_render::particles::TABLE_SAMPLES;
use voltra_render::SpawnShape;

use crate::{Collider2D, Transform2D};

/// The area or line an emitter's newborns are spread over, in its space.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum EmitShape {
    /// Every one at the emitter's origin.
    #[default]
    Point,
    /// Evenly over a disc.
    Circle {
        /// The disc's radius.
        radius: f32,
    },
    /// Evenly over the band between two circles; equal radii are a circle's
    /// edge.
    Ring {
        /// The inner radius.
        inner: f32,
        /// The outer radius.
        outer: f32,
    },
    /// Evenly over a rectangle.
    Box {
        /// Half its width and height.
        half_extents: Vec2,
    },
    /// Evenly along a line on the emitter's x axis, through its origin.
    Edge {
        /// Half its length.
        half_length: f32,
    },
    /// Evenly along the outline of the entity's own [`Collider2D`], as the
    /// collider sits at the origin. At the origin when the entity has none.
    Outline,
}

impl EmitShape {
    /// Every kind, as an editor lists them, with their default sizes.
    pub const ALL: [Self; 6] = [
        Self::Point,
        Self::Circle { radius: 0.5 },
        Self::Ring {
            inner: 0.4,
            outer: 0.5,
        },
        Self::Box {
            half_extents: Vec2::splat(0.5),
        },
        Self::Edge { half_length: 0.5 },
        Self::Outline,
    ];

    /// Whether it is a point: what a scene file leaves unwritten.
    pub fn is_point(&self) -> bool {
        *self == Self::Point
    }

    /// What an editor calls it.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Point => "point",
            Self::Circle { .. } => "circle",
            Self::Ring { .. } => "ring",
            Self::Box { .. } => "box",
            Self::Edge { .. } => "edge",
            Self::Outline => "collider outline",
        }
    }

    /// The shape as the GPU simulation takes it.
    pub(crate) fn spawn(&self) -> SpawnShape {
        match *self {
            Self::Point => SpawnShape::Point,
            Self::Circle { radius } => SpawnShape::Ring {
                inner: 0.0,
                outer: radius,
            },
            Self::Ring { inner, outer } => SpawnShape::Ring { inner, outer },
            Self::Box { half_extents } => SpawnShape::Box { half_extents },
            Self::Edge { half_length } => SpawnShape::Edge { half_length },
            Self::Outline => SpawnShape::Outline,
        }
    }

    /// A newborn's offset from the origin, from two values in `0..1` —
    /// `particles_sim.wgsl`'s `spawn_offset`, line for line. `outline` is
    /// [`outline_table`]'s; an outline of fewer than two corners is the
    /// origin, as is anything not finite.
    pub(crate) fn offset(&self, outline: &[[f32; 4]], a: f32, b: f32) -> Vec2 {
        let offset = match self.spawn() {
            SpawnShape::Point => Vec2::ZERO,
            SpawnShape::Ring { inner, outer } => {
                let (inner, outer) = (inner.abs().min(outer.abs()), inner.abs().max(outer.abs()));
                let r = (inner * inner + (outer * outer - inner * inner) * a).sqrt();
                Vec2::from_angle(b * std::f32::consts::TAU) * r
            }
            SpawnShape::Box { half_extents } => {
                Vec2::new(a * 2.0 - 1.0, b * 2.0 - 1.0) * half_extents.abs()
            }
            SpawnShape::Edge { half_length } => Vec2::new((a * 2.0 - 1.0) * half_length.abs(), 0.0),
            SpawnShape::Outline if outline.len() >= 2 => on_outline(outline, a),
            SpawnShape::Outline => Vec2::ZERO,
        };
        if offset.is_finite() {
            offset
        } else {
            Vec2::ZERO
        }
    }

    /// The farthest a newborn can start from the origin: what the GPU's
    /// culling box grows by.
    pub(crate) fn extent(&self, outline: &[[f32; 4]]) -> f32 {
        let extent = match *self {
            Self::Point => 0.0,
            Self::Circle { radius } => radius.abs(),
            Self::Ring { inner, outer } => inner.abs().max(outer.abs()),
            Self::Box { half_extents } => half_extents.abs().length(),
            Self::Edge { half_length } => half_length.abs(),
            Self::Outline => outline
                .iter()
                .map(|c| Vec2::new(c[0], c[1]).length())
                .fold(0.0, f32::max),
        };
        if extent.is_finite() {
            extent
        } else {
            f32::INFINITY
        }
    }
}

/// The outline of `entity`'s collider at the origin, as
/// [`outline_table`] lays it out — or empty without a collider.
pub(crate) fn collider_outline(collider: Option<&Collider2D>) -> Vec<[f32; 4]> {
    // A full turn of a round part in 32 segments: smooth at a particle's
    // size, and well inside the table's room.
    collider.map_or_else(Vec::new, |collider| {
        outline_table(&collider.outline(&Transform2D::default(), 32))
    })
}

/// A closed outline as both simulations read it: each corner with the
/// fraction of the perimeter before it. At most
/// [`TABLE_SAMPLES`] corners — a longer one keeps every few, evenly — and
/// none that are not finite. Empty for an outline with no length.
pub fn outline_table(points: &[Vec2]) -> Vec<[f32; 4]> {
    let points: Vec<Vec2> = points.iter().copied().filter(|p| p.is_finite()).collect();
    let points: Vec<Vec2> = if points.len() > TABLE_SAMPLES {
        (0..TABLE_SAMPLES)
            .map(|i| points[i * points.len() / TABLE_SAMPLES])
            .collect()
    } else {
        points
    };
    let n = points.len();
    let lengths: Vec<f32> = (0..n)
        .map(|i| points[i].distance(points[(i + 1) % n]))
        .collect();
    let perimeter: f32 = lengths.iter().sum();
    if n < 2 || !(perimeter > 0.0 && perimeter.is_finite()) {
        return Vec::new();
    }
    let mut before = 0.0;
    points
        .iter()
        .zip(&lengths)
        .map(|(point, length)| {
            let corner = [point.x, point.y, before / perimeter, 0.0];
            before += length;
            corner
        })
        .collect()
}

/// The point `u` (`0..1`) of the way round `outline`, walking from its first
/// corner — `particles_sim.wgsl`'s `on_outline`.
pub(crate) fn on_outline(outline: &[[f32; 4]], u: f32) -> Vec2 {
    let n = outline.len();
    for i in 0..n {
        let here = outline[i];
        let next = outline[(i + 1) % n];
        let end = if i + 1 == n { 1.0 } else { next[2] };
        if u < end || i + 1 == n {
            let span = end - here[2];
            let f = if span > 0.0 {
                ((u - here[2]) / span).clamp(0.0, 1.0)
            } else {
                0.0
            };
            return Vec2::new(here[0], here[1]).lerp(Vec2::new(next[0], next[1]), f);
        }
    }
    Vec2::ZERO
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<[f32; 4]> {
        outline_table(&[
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
        ])
    }

    #[test]
    fn an_outline_is_walked_by_its_length() {
        let table = square();
        assert_eq!(table[2][2], 0.5);
        assert_eq!(on_outline(&table, 0.0), Vec2::new(1.0, 1.0));
        assert_eq!(on_outline(&table, 0.125), Vec2::new(0.0, 1.0));
        assert_eq!(on_outline(&table, 0.875), Vec2::new(1.0, 0.0));
    }

    #[test]
    fn an_outline_with_no_length_is_none() {
        assert!(outline_table(&[Vec2::ONE, Vec2::ONE]).is_empty());
        assert!(outline_table(&[Vec2::ONE]).is_empty());
        assert_eq!(EmitShape::Outline.offset(&[], 0.3, 0.3), Vec2::ZERO);
    }

    #[test]
    fn a_long_outline_is_thinned_to_the_table() {
        let circle: Vec<Vec2> = (0..200)
            .map(|i| Vec2::from_angle(i as f32 / 200.0 * std::f32::consts::TAU))
            .collect();
        assert_eq!(outline_table(&circle).len(), TABLE_SAMPLES);
    }

    #[test]
    fn shapes_place_their_newborns_inside_themselves() {
        let ring = EmitShape::Ring {
            inner: 1.0,
            outer: 2.0,
        };
        for (a, b) in [(0.0, 0.0), (1.0, 0.5), (0.5, 0.9)] {
            let r = ring.offset(&[], a, b).length();
            assert!((1.0 - 1e-5..=2.0 + 1e-5).contains(&r), "{r}");
        }
        let edge = EmitShape::Edge { half_length: 3.0 };
        assert_eq!(edge.offset(&[], 1.0, 0.7), Vec2::new(3.0, 0.0));
        let boxed = EmitShape::Box {
            half_extents: Vec2::new(1.0, 2.0),
        };
        assert_eq!(boxed.offset(&[], 0.0, 1.0), Vec2::new(-1.0, 2.0));
        assert_eq!(boxed.extent(&[]), Vec2::new(1.0, 2.0).length());
    }

    #[test]
    fn a_collider_s_outline_is_where_it_is_at_the_origin() {
        let collider = Collider2D::Box {
            half_extents: Vec2::new(2.0, 1.0),
        };
        let table = collider_outline(Some(&collider));
        assert_eq!(table.len(), 4);
        assert_eq!(
            EmitShape::Outline.extent(&table),
            Vec2::new(2.0, 1.0).length()
        );
        assert!(collider_outline(None).is_empty());
    }
}
