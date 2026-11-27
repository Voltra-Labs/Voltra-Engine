//! Any collider, placed: a convex outline of world-space points grown by a
//! radius.
//!
//! Box2D v3's one representation for every shape, and the reason the
//! physics needs one algorithm per *kind* of pair rather than per pair of
//! variants: a box is four points and no radius, a polygon its points and
//! its rounding, a capsule the two centres of its caps and their radius, a
//! circle one point and its radius.

use voltra_render::glam::Vec2;

use super::ConvexPolygon;

/// A collider in world space: the convex hull of [`Self::vertices`], grown
/// by [`Self::radius`] all round.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldShape {
    vertices: [Vec2; ConvexPolygon::MAX_POINTS],
    count: u8,
    radius: f32,
}

impl WorldShape {
    /// The shape around `vertices` — at most [`ConvexPolygon::MAX_POINTS`],
    /// counter-clockwise when there are three or more — grown by `radius`.
    ///
    /// # Panics
    ///
    /// With no vertices or more than the maximum: every collider has between
    /// one and eight, and a caller handing over another count is a bug.
    pub fn new(vertices: &[Vec2], radius: f32) -> Self {
        assert!(
            (1..=ConvexPolygon::MAX_POINTS).contains(&vertices.len()),
            "a shape has one to eight vertices, not {}",
            vertices.len()
        );
        let mut stored = [Vec2::ZERO; ConvexPolygon::MAX_POINTS];
        stored[..vertices.len()].copy_from_slice(vertices);
        Self {
            vertices: stored,
            count: vertices.len() as u8,
            radius,
        }
    }

    /// The hull's corners: one for a circle, two for a capsule's caps,
    /// counter-clockwise from three on.
    pub fn vertices(&self) -> &[Vec2] {
        &self.vertices[..self.count as usize]
    }

    /// How far the surface stands off the hull.
    pub fn radius(&self) -> f32 {
        self.radius
    }

    /// The world bounds, as `(min, max)`.
    pub fn bounds(&self) -> (Vec2, Vec2) {
        let first = self.vertices[0];
        let (min, max) = self
            .vertices()
            .iter()
            .fold((first, first), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
        // Clamped so a nonsense radius gives the hull's box rather than an
        // inverted one; `Collider2D::is_degenerate` is what rejects it.
        let grow = Vec2::splat(self.radius.max(0.0));
        (min - grow, max + grow)
    }

    /// The surface as a closed polyline, counter-clockwise, each rounded
    /// corner cut so that a full turn takes `sides` segments.
    ///
    /// Every corner is an arc from the normal of the edge arriving to the
    /// normal of the edge leaving, so a capsule's caps are half turns and a
    /// circle — one corner, no edges — is the whole turn.
    pub fn outline(&self, sides: usize) -> Vec<Vec2> {
        let vertices = self.vertices();
        let radius = self.radius.max(0.0);
        if radius == 0.0 && vertices.len() > 1 {
            return vertices.to_vec();
        }
        let sides = sides.max(3);
        let step = std::f32::consts::TAU / sides as f32;
        if vertices.len() == 1 {
            return (0..sides)
                .map(|i| vertices[0] + Vec2::from_angle(i as f32 * step) * radius)
                .collect();
        }
        let n = vertices.len();
        let normal = |i: usize| {
            let edge = vertices[(i + 1) % n] - vertices[i];
            Vec2::new(edge.y, -edge.x).normalize_or_zero()
        };
        let mut points = Vec::new();
        for (i, vertex) in vertices.iter().enumerate() {
            let (arriving, leaving) = (normal((i + n - 1) % n), normal(i));
            let start = arriving.to_angle();
            let mut sweep = leaving.to_angle() - start;
            if sweep < 0.0 {
                sweep += std::f32::consts::TAU;
            }
            let cuts = (sweep / step).ceil().max(1.0) as usize;
            points.extend((0..=cuts).map(|k| {
                let angle = start + sweep * k as f32 / cuts as f32;
                *vertex + Vec2::from_angle(angle) * radius
            }));
        }
        points
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(points: &[Vec2]) -> f32 {
        let n = points.len();
        0.5 * (0..n)
            .map(|i| points[i].perp_dot(points[(i + 1) % n]))
            .sum::<f32>()
    }

    #[test]
    fn a_capsules_outline_is_a_rectangle_and_two_half_circles() {
        let shape = WorldShape::new(&[Vec2::new(0.0, -1.0), Vec2::new(0.0, 1.0)], 0.5);
        let outline = shape.outline(64);
        let expected = 2.0 * 1.0 + std::f32::consts::PI * 0.25;

        assert!(
            (area(&outline) - expected).abs() < 0.01,
            "{}",
            area(&outline)
        );
        for point in &outline {
            let on_segment = Vec2::new(0.0, point.y.clamp(-1.0, 1.0));
            assert!(
                ((*point - on_segment).length() - 0.5).abs() < 1e-4,
                "{point:?}"
            );
        }
    }

    #[test]
    fn a_sharp_polygons_outline_is_its_corners() {
        let corners = [Vec2::ZERO, Vec2::X, Vec2::Y];
        assert_eq!(WorldShape::new(&corners, 0.0).outline(24), corners.to_vec());
    }

    #[test]
    fn a_rounded_polygons_bounds_and_outline_grow_by_the_radius() {
        let shape = WorldShape::new(&[Vec2::ZERO, Vec2::X, Vec2::Y], 0.25);
        let (min, max) = shape.bounds();

        assert_eq!(min, Vec2::splat(-0.25));
        assert_eq!(max, Vec2::splat(1.25));
        assert!(
            area(&shape.outline(48)) > 0.5,
            "counter-clockwise and grown"
        );
    }
}
