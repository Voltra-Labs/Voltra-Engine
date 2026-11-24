//! How far apart two convex shapes are: GJK over their corners.
//!
//! Box2D's `b2ShapeDistance`, which is Gino van den Bergen's algorithm with
//! Erin Catto's simplex solver: walk a simplex of the Minkowski difference
//! `B − A` towards the origin, a corner at a time, until no corner brings it
//! nearer. The radii are left out — callers subtract them — so the cores
//! are what is measured, and a circle is a single point.

use voltra_render::glam::Vec2;

/// More than enough for shapes of eight corners; a walk that has not settled
/// by then is cycling on rounding, and its last answer is kept.
const MAX_ITERATIONS: usize = 24;

/// The nearest points of two convex point sets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Closest {
    /// On the first shape.
    pub a: Vec2,
    /// On the second.
    pub b: Vec2,
    /// Zero when the hulls overlap.
    pub distance: f32,
    /// The corner pairs `(on a, on b)` the nearest points are made of: one
    /// for corner against corner, two for a corner against an edge.
    features: [(usize, usize); 2],
    feature_count: usize,
}

impl Closest {
    /// The unit vector from `b` to `a`: what pushes the first shape away from
    /// the second. `None` when they touch or overlap and there is no direction.
    pub(crate) fn normal(&self) -> Option<Vec2> {
        (self.a - self.b).try_normalize()
    }

    /// [`Self::normal`], taken from the edge the nearest points lie on when
    /// they do: the face's own normal, oriented from `b` to `a`.
    ///
    /// Two shapes a hair apart have nearest points whose difference is
    /// mostly rounding, and its direction with it; the face they lie
    /// against has an exact one. Box2D's time of impact separates along the
    /// face for the same reason.
    pub(crate) fn face_normal(&self, a: &[Vec2], b: &[Vec2]) -> Option<Vec2> {
        let towards = self.normal()?;
        if self.feature_count != 2 {
            return Some(towards);
        }
        let [(a0, b0), (a1, b1)] = self.features;
        let edge = if a0 == a1 {
            b[b1] - b[b0]
        } else if b0 == b1 {
            a[a1] - a[a0]
        } else {
            // Two edges alongside each other: either is the face.
            b[b1] - b[b0]
        };
        let Some(face) = edge.perp().try_normalize() else {
            return Some(towards);
        };
        Some(if face.dot(towards) < 0.0 { -face } else { face })
    }
}

#[derive(Debug, Clone, Copy)]
struct Corner {
    a: Vec2,
    b: Vec2,
    /// `b − a`: a point of the Minkowski difference.
    w: Vec2,
    /// Barycentric weight.
    weight: f32,
    index_a: usize,
    index_b: usize,
}

impl Corner {
    fn new(a: &[Vec2], b: &[Vec2], index_a: usize, index_b: usize) -> Self {
        Self {
            a: a[index_a],
            b: b[index_b],
            w: b[index_b] - a[index_a],
            weight: 1.0,
            index_a,
            index_b,
        }
    }
}

/// The nearest points of the hulls of `a` and `b`.
///
/// # Panics
///
/// If either is empty: every shape has at least one corner.
pub(crate) fn closest(a: &[Vec2], b: &[Vec2]) -> Closest {
    assert!(!a.is_empty() && !b.is_empty(), "a shape has a corner");
    let mut simplex = [Corner::new(a, b, 0, 0); 3];
    let mut count = 1;

    for _ in 0..MAX_ITERATIONS {
        let mut saved = [(usize::MAX, usize::MAX); 3];
        for (slot, corner) in saved.iter_mut().zip(&simplex[..count]) {
            *slot = (corner.index_a, corner.index_b);
        }
        count = match count {
            2 => solve2(&mut simplex),
            3 => solve3(&mut simplex),
            _ => 1,
        };
        if count == 3 {
            // The origin is inside the triangle: the shapes overlap.
            break;
        }
        let direction = search_direction(&simplex[..count]);
        if direction.length_squared() < f32::EPSILON * f32::EPSILON {
            // The origin is on the simplex: touching, to rounding.
            break;
        }
        let corner = Corner::new(a, b, support(a, -direction), support(b, direction));
        if saved.contains(&(corner.index_a, corner.index_b)) {
            // No new corner: this is as near as it gets.
            break;
        }
        simplex[count] = corner;
        count += 1;
    }

    let (pa, pb) = match count {
        1 => (simplex[0].a, simplex[0].b),
        2 => (
            simplex[0].a * simplex[0].weight + simplex[1].a * simplex[1].weight,
            simplex[0].b * simplex[0].weight + simplex[1].b * simplex[1].weight,
        ),
        _ => {
            let p = simplex[0].a * simplex[0].weight
                + simplex[1].a * simplex[1].weight
                + simplex[2].a * simplex[2].weight;
            (p, p)
        }
    };
    let mut features = [(0, 0); 2];
    for (slot, corner) in features.iter_mut().zip(&simplex[..count.min(2)]) {
        *slot = (corner.index_a, corner.index_b);
    }
    Closest {
        a: pa,
        b: pb,
        distance: if count == 3 { 0.0 } else { pa.distance(pb) },
        features,
        feature_count: count,
    }
}

fn support(points: &[Vec2], direction: Vec2) -> usize {
    let mut best = 0;
    let mut most = points[0].dot(direction);
    for (i, point) in points.iter().enumerate().skip(1) {
        let along = point.dot(direction);
        if along > most {
            most = along;
            best = i;
        }
    }
    best
}

/// From the simplex towards the origin.
fn search_direction(simplex: &[Corner]) -> Vec2 {
    match simplex {
        [one] => -one.w,
        [one, two] => {
            let edge = two.w - one.w;
            // Which side of the edge the origin is on.
            if edge.perp_dot(-one.w) > 0.0 {
                edge.perp()
            } else {
                -edge.perp()
            }
        }
        _ => Vec2::ZERO,
    }
}

/// The part of the segment nearest the origin: an end, or the inside.
fn solve2(s: &mut [Corner; 3]) -> usize {
    let (w1, w2) = (s[0].w, s[1].w);
    let edge = w2 - w1;
    let d12_2 = -w1.dot(edge);
    if d12_2 <= 0.0 {
        s[0].weight = 1.0;
        return 1;
    }
    let d12_1 = w2.dot(edge);
    if d12_1 <= 0.0 {
        s[0] = s[1];
        s[0].weight = 1.0;
        return 1;
    }
    let inverse = 1.0 / (d12_1 + d12_2);
    s[0].weight = d12_1 * inverse;
    s[1].weight = d12_2 * inverse;
    2
}

/// The part of the triangle nearest the origin: a corner, an edge, or the
/// inside, by the signs of the barycentric coordinates.
fn solve3(s: &mut [Corner; 3]) -> usize {
    let (w1, w2, w3) = (s[0].w, s[1].w, s[2].w);

    let e12 = w2 - w1;
    let (d12_1, d12_2) = (w2.dot(e12), -w1.dot(e12));
    let e13 = w3 - w1;
    let (d13_1, d13_2) = (w3.dot(e13), -w1.dot(e13));
    let e23 = w3 - w2;
    let (d23_1, d23_2) = (w3.dot(e23), -w2.dot(e23));

    let n123 = e12.perp_dot(e13);
    let d123_1 = n123 * w2.perp_dot(w3);
    let d123_2 = n123 * w3.perp_dot(w1);
    let d123_3 = n123 * w1.perp_dot(w2);

    if d12_2 <= 0.0 && d13_2 <= 0.0 {
        s[0].weight = 1.0;
        return 1;
    }
    if d12_1 > 0.0 && d12_2 > 0.0 && d123_3 <= 0.0 {
        let inverse = 1.0 / (d12_1 + d12_2);
        s[0].weight = d12_1 * inverse;
        s[1].weight = d12_2 * inverse;
        return 2;
    }
    if d13_1 > 0.0 && d13_2 > 0.0 && d123_2 <= 0.0 {
        let inverse = 1.0 / (d13_1 + d13_2);
        s[0].weight = d13_1 * inverse;
        s[2].weight = d13_2 * inverse;
        s[1] = s[2];
        return 2;
    }
    if d12_1 <= 0.0 && d23_2 <= 0.0 {
        s[0] = s[1];
        s[0].weight = 1.0;
        return 1;
    }
    if d13_1 <= 0.0 && d23_1 <= 0.0 {
        s[0] = s[2];
        s[0].weight = 1.0;
        return 1;
    }
    if d23_1 > 0.0 && d23_2 > 0.0 && d123_1 <= 0.0 {
        let inverse = 1.0 / (d23_1 + d23_2);
        s[1].weight = d23_1 * inverse;
        s[2].weight = d23_2 * inverse;
        s[0] = s[2];
        return 2;
    }
    let inverse = 1.0 / (d123_1 + d123_2 + d123_3);
    s[0].weight = d123_1 * inverse;
    s[1].weight = d123_2 * inverse;
    s[2].weight = d123_3 * inverse;
    3
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(at: Vec2) -> Vec<Vec2> {
        [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)]
            .map(|(x, y)| at + Vec2::new(x, y))
            .to_vec()
    }

    #[test]
    fn two_squares_side_by_side_are_their_gap_apart() {
        let result = closest(&square(Vec2::ZERO), &square(Vec2::new(3.5, 0.4)));

        assert!((result.distance - 1.5).abs() < 1e-5, "{result:?}");
        let normal = result.normal().expect("apart");
        assert!((normal - Vec2::NEG_X).length() < 1e-5, "{normal:?}");
    }

    #[test]
    fn corner_to_corner_is_the_diagonal_gap() {
        let result = closest(&square(Vec2::ZERO), &square(Vec2::new(3.0, 3.0)));

        assert!((result.distance - 2f32.sqrt()).abs() < 1e-5, "{result:?}");
        assert!((result.a - Vec2::ONE).length() < 1e-5, "{result:?}");
    }

    #[test]
    fn overlapping_squares_are_no_distance_apart() {
        let result = closest(&square(Vec2::ZERO), &square(Vec2::new(1.0, 0.5)));
        assert_eq!(result.distance, 0.0, "{result:?}");
    }

    #[test]
    fn a_point_and_a_segment() {
        let result = closest(
            &[Vec2::new(0.0, 2.0)],
            &[Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0)],
        );

        assert!((result.distance - 2.0).abs() < 1e-6, "{result:?}");
        assert!(result.b.length() < 1e-6, "{result:?}");
    }

    #[test]
    fn a_point_inside_a_square_is_in_it() {
        let result = closest(&[Vec2::new(0.3, -0.2)], &square(Vec2::ZERO));
        assert_eq!(result.distance, 0.0, "{result:?}");
    }
}
