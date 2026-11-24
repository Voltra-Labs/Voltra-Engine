//! A placed shape with what the narrow phase reads off it: its edges'
//! outward normals.

use voltra_render::glam::Vec2;
use voltra_scene::{Collider, ConvexPolygon, Transform, WorldShape};

/// A collider in world space: corners, the outward normal of the edge
/// leaving each, and the radius the surface stands off them.
///
/// One corner is a circle, and has no edges. Two are a capsule's caps: the
/// edge there and the edge back, normals opposite. Three or more wind
/// counter-clockwise, so the outward normal of `p → q` is `(d.y, -d.x)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Hull {
    vertices: [Vec2; ConvexPolygon::MAX_POINTS],
    normals: [Vec2; ConvexPolygon::MAX_POINTS],
    count: usize,
    pub radius: f32,
}

impl Hull {
    pub(crate) fn of(collider: &Collider, transform: &Transform) -> Self {
        Self::from_shape(&collider.world_shape(transform))
    }

    pub(crate) fn from_shape(shape: &WorldShape) -> Self {
        let mut vertices = [Vec2::ZERO; ConvexPolygon::MAX_POINTS];
        let mut normals = [Vec2::ZERO; ConvexPolygon::MAX_POINTS];
        let points = shape.vertices();
        let count = points.len();
        vertices[..count].copy_from_slice(points);
        if count >= 2 {
            for i in 0..count {
                let edge = vertices[(i + 1) % count] - vertices[i];
                // A zero-length edge cannot come out of a hull or a capsule
                // that passed `is_degenerate`; zero rather than NaN if one does.
                normals[i] = Vec2::new(edge.y, -edge.x).normalize_or_zero();
            }
        }
        Self {
            vertices,
            normals,
            count,
            radius: shape.radius(),
        }
    }

    pub(crate) fn vertices(&self) -> &[Vec2] {
        &self.vertices[..self.count]
    }

    pub(crate) fn normals(&self) -> &[Vec2] {
        &self.normals[..self.count]
    }

    pub(crate) fn len(&self) -> usize {
        self.count
    }

    /// The vertex after `i`, wrapping.
    pub(crate) fn next(&self, i: usize) -> usize {
        if i + 1 < self.count {
            i + 1
        } else {
            0
        }
    }

    /// The same shape moved by `offset`.
    pub(crate) fn moved(&self, offset: Vec2) -> Self {
        let mut moved = *self;
        for vertex in &mut moved.vertices[..self.count] {
            *vertex += offset;
        }
        moved
    }

    /// The point of the core nearest `point`, and whether `point` is inside
    /// the core — only a hull of three or more corners has an inside.
    pub(crate) fn nearest(&self, point: Vec2) -> (Vec2, bool) {
        let v = self.vertices();
        match v.len() {
            1 => (v[0], false),
            2 => (nearest_on_segment(v[0], v[1], point), false),
            _ => {
                let inside = (0..v.len()).all(|i| self.normals[i].dot(point - v[i]) <= 0.0);
                if inside {
                    return (point, true);
                }
                let nearest = (0..v.len())
                    .map(|i| nearest_on_segment(v[i], v[self.next(i)], point))
                    .min_by(|a, b| {
                        a.distance_squared(point)
                            .total_cmp(&b.distance_squared(point))
                    })
                    .expect("a hull has corners");
                (nearest, false)
            }
        }
    }

    /// The outward normal of the surface at `point`, which lies on it: away
    /// from the core where the surface is round, the face's where it is flat.
    pub(crate) fn normal_at(&self, point: Vec2, fallback: Vec2) -> Vec2 {
        let (nearest, inside) = self.nearest(point);
        let away = point - nearest;
        if !inside && away.length_squared() > (self.radius * 1e-3).powi(2).max(1e-12) {
            return away.normalize();
        }
        // On a flat face: the face the point is furthest out of.
        let v = self.vertices();
        if v.len() < 2 {
            return away.try_normalize().unwrap_or(fallback);
        }
        (0..v.len())
            .map(|i| (self.normals[i], self.normals[i].dot(point - v[i])))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map_or(fallback, |(normal, _)| normal)
    }
}

/// The point of the segment `a → b` nearest `point`.
pub(crate) fn nearest_on_segment(a: Vec2, b: Vec2, point: Vec2) -> Vec2 {
    let edge = b - a;
    let length = edge.length_squared();
    if length <= f32::MIN_POSITIVE {
        return a;
    }
    a + edge * ((point - a).dot(edge) / length).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capsules_two_edges_face_opposite_ways() {
        let capsule = Collider::Capsule {
            half_height: 1.0,
            radius: 0.5,
        };
        let hull = Hull::of(&capsule, &Transform::default());

        assert_eq!(hull.len(), 2);
        assert!((hull.normals()[0] + hull.normals()[1]).length() < 1e-6);
        assert!((hull.normals()[0].length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_boxs_normals_point_out() {
        let hull = Hull::of(&Collider::default(), &Transform::default());
        for (vertex, normal) in hull.vertices().iter().zip(hull.normals()) {
            // Each normal leaves its edge away from the centre.
            assert!(normal.dot(*vertex) > 0.0, "{normal:?} at {vertex:?}");
        }
    }

    #[test]
    fn the_normal_on_a_round_part_points_away_from_the_core() {
        let capsule = Collider::Capsule {
            half_height: 1.0,
            radius: 0.5,
        };
        let hull = Hull::of(&capsule, &Transform::default());
        let top = hull.normal_at(Vec2::new(0.0, 1.0), Vec2::X);
        let side = hull.normal_at(Vec2::new(0.5, 0.0), Vec2::Y);

        assert!((top - Vec2::Y).length() < 1e-5, "{top:?}");
        assert!((side - Vec2::X).length() < 1e-5, "{side:?}");
    }
}
