//! A simple polygon cut into triangles by ear clipping.
//!
//! The textbook algorithm (Eberly, *Triangulation by Ear Clipping*), as
//! Godot's `Geometry2D::triangulate_polygon` and Unity's sprite tessellator
//! fall back to: repeatedly cut off a corner whose triangle is convex and
//! holds no other corner. O(n²) for the hundreds of points an authored
//! shape has, any winding, concave shapes included. Repeated points and
//! points on a straight line are dropped first; a polygon that crosses
//! itself has no ears left at some point, and what remains is fanned so
//! something is still drawn — and the caller is told.

use voltra_render::glam::Vec2;

/// How a triangulation went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Triangulation {
    /// Three indices per triangle, into the points given.
    pub indices: Vec<u32>,
    /// Whether every triangle was an ear. `false` for a polygon that
    /// crosses itself, whose last part was fanned instead.
    pub clean: bool,
}

/// Twice the signed area of `points`: positive counter-clockwise.
pub fn signed_area2(points: &[Vec2]) -> f32 {
    (0..points.len())
        .map(|i| points[i].perp_dot(points[(i + 1) % points.len()]))
        .sum()
}

/// Triangles covering the polygon `points` (closed, either winding), as
/// indices into `points`. Fewer than three usable points is no triangle.
pub fn triangulate(points: &[Vec2]) -> Triangulation {
    // The corners that make the shape: finite, not a repeat of the one
    // before, not on the line between their neighbours.
    let mut ring: Vec<usize> = (0..points.len())
        .filter(|&i| points[i].is_finite())
        .collect();
    ring.dedup_by(|b, a| points[*a].distance_squared(points[*b]) <= f32::EPSILON);
    while ring.len() > 1
        && points[ring[0]].distance_squared(points[ring[ring.len() - 1]]) <= f32::EPSILON
    {
        ring.pop();
    }
    drop_straight(points, &mut ring);
    let mut indices = Vec::new();
    if ring.len() < 3 {
        return Triangulation {
            indices,
            clean: true,
        };
    }
    let polygon: Vec<Vec2> = ring.iter().map(|&i| points[i]).collect();
    let ccw = signed_area2(&polygon) > 0.0;
    let mut clean = true;
    while ring.len() > 3 {
        let n = ring.len();
        let ear = (0..n).find(|&i| is_ear(points, &ring, i, ccw));
        let Some(i) = ear else {
            clean = false;
            break;
        };
        let (a, b, c) = (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
        indices.extend([a as u32, b as u32, c as u32]);
        ring.remove(i);
        drop_straight(points, &mut ring);
    }
    if ring.len() >= 3 {
        // The last triangle, or a fan over what no ear could be cut from.
        for k in 1..ring.len() - 1 {
            indices.extend([ring[0] as u32, ring[k] as u32, ring[k + 1] as u32]);
        }
    }
    Triangulation { indices, clean }
}

/// Removes corners that lie on the line between their neighbours.
fn drop_straight(points: &[Vec2], ring: &mut Vec<usize>) {
    let mut i = 0;
    while ring.len() >= 3 && i < ring.len() {
        let n = ring.len();
        let (a, b, c) = (
            points[ring[(i + n - 1) % n]],
            points[ring[i]],
            points[ring[(i + 1) % n]],
        );
        let scale = (b - a).length() * (c - b).length();
        if (b - a).perp_dot(c - b).abs() <= 1e-6 * scale.max(f32::MIN_POSITIVE) {
            ring.remove(i);
            i = i.saturating_sub(1);
        } else {
            i += 1;
        }
    }
}

/// Whether corner `i` of `ring` is an ear: convex in the polygon's winding,
/// with no other corner inside or on its triangle.
fn is_ear(points: &[Vec2], ring: &[usize], i: usize, ccw: bool) -> bool {
    let n = ring.len();
    let (ia, ib, ic) = (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
    let (a, b, c) = (points[ia], points[ib], points[ic]);
    let turn = (b - a).perp_dot(c - b);
    if (turn > 0.0) != ccw || turn == 0.0 {
        return false;
    }
    ring.iter()
        .filter(|&&j| j != ia && j != ib && j != ic)
        .all(|&j| !inside(points[j], a, b, c))
}

/// Whether `p` is inside or on triangle `a b c`, either winding.
fn inside(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let d1 = (b - a).perp_dot(p - a);
    let d2 = (c - b).perp_dot(p - b);
    let d3 = (a - c).perp_dot(p - c);
    let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(negative && positive)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(points: &[Vec2], indices: &[u32]) -> f32 {
        indices
            .chunks(3)
            .map(|t| {
                let [a, b, c] = [0, 1, 2].map(|k| points[t[k] as usize]);
                (b - a).perp_dot(c - a).abs() * 0.5
            })
            .sum()
    }

    #[test]
    fn a_concave_polygon_is_covered_exactly_either_way_round() {
        // An L: three unit squares.
        let mut l = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(2.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 2.0),
            Vec2::new(0.0, 2.0),
        ];
        for _ in 0..2 {
            let cut = triangulate(&l);
            assert!(cut.clean);
            assert_eq!(cut.indices.len(), 4 * 3);
            assert!((area(&l, &cut.indices) - 3.0).abs() < 1e-5);
            l.reverse();
        }
    }

    #[test]
    fn repeats_and_straight_corners_are_dropped() {
        let square = [
            Vec2::ZERO,
            Vec2::ZERO,
            Vec2::new(0.5, 0.0),
            Vec2::X,
            Vec2::ONE,
            Vec2::Y,
            Vec2::ZERO,
        ];
        let cut = triangulate(&square);
        assert_eq!(cut.indices.len(), 6, "two triangles for a square");
        assert!((area(&square, &cut.indices) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn too_few_points_or_a_line_is_nothing() {
        assert!(triangulate(&[Vec2::ZERO, Vec2::X]).indices.is_empty());
        let line = [Vec2::ZERO, Vec2::X, Vec2::new(2.0, 0.0)];
        assert!(triangulate(&line).indices.is_empty());
        assert!(triangulate(&[Vec2::NAN, Vec2::X, Vec2::Y])
            .indices
            .is_empty());
    }

    #[test]
    fn a_polygon_crossing_itself_still_draws_and_says_so() {
        let bow = [
            Vec2::ZERO,
            Vec2::new(2.0, 2.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(0.0, 2.0),
        ];
        let cut = triangulate(&bow);
        assert!(!cut.indices.is_empty());
        let _ = cut.clean;
    }

    #[test]
    fn a_star_has_its_points_and_not_its_notches() {
        let star: Vec<Vec2> = (0..10)
            .map(|i| {
                let r = if i % 2 == 0 { 2.0 } else { 0.8 };
                Vec2::from_angle(i as f32 * std::f32::consts::TAU / 10.0) * r
            })
            .collect();
        let cut = triangulate(&star);
        assert!(cut.clean);
        assert_eq!(cut.indices.len(), 8 * 3);
        assert!((area(&star, &cut.indices) - signed_area2(&star).abs() * 0.5).abs() < 1e-4);
    }
}
