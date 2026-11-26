//! Weights over a plane: the triangle of children around the input, by
//! barycentric weights — or the nearest edge, outside them all.

use super::line;

/// How close, as a fraction of the children's spread, two positions are
/// one, three points are a line, or a point is on a circle or an edge.
const EPSILON: f64 = 1e-9;

/// Writes into `out` each of `n` children's weight for the input `at`,
/// child `i` sitting at `position(i)` — Unity's simple directional blend,
/// on any layout.
///
/// The children's positions are triangulated (Delaunay: the triangles with
/// no other child inside their circumcircle, so a centre child surrounded
/// by directions makes the fan of triangles a directional blend uses). An
/// input inside a triangle mixes its three corners by barycentric weights:
/// at a child, that child alone; on an edge, the edge's two. Outside every
/// triangle it is clamped to the nearest point of the hull and mixes that
/// edge's two (or the one corner there). Children all on one line blend
/// along it as a 1D blend does; children at one position share its weight
/// equally; a position or an input that is not finite weighs nothing or
/// reads as zero. The weights sum to one unless no position is finite.
pub(crate) fn weights(
    n: usize,
    position: impl Fn(usize) -> [f32; 2],
    at: [f32; 2],
    out: &mut [f32],
) {
    out.fill(0.0);
    let at = at.map(|v| if v.is_finite() { v } else { 0.0 });
    let finite: Vec<usize> = (0..n)
        .filter(|&i| position(i).iter().all(|v| v.is_finite()))
        .collect();
    if finite.is_empty() {
        return;
    }
    // Into a unit-ish frame, so one epsilon holds at any scale.
    let raw = |i: usize| position(i).map(f64::from);
    let (mut low, mut high) = ([f64::MAX; 2], [f64::MIN; 2]);
    for &i in &finite {
        let p = raw(i);
        for axis in 0..2 {
            low[axis] = low[axis].min(p[axis]);
            high[axis] = high[axis].max(p[axis]);
        }
    }
    let at64 = at.map(f64::from);
    let centre = [(low[0] + high[0]) / 2.0, (low[1] + high[1]) / 2.0];
    let spread = (high[0] - low[0])
        .max(high[1] - low[1])
        .max(f64::MIN_POSITIVE);
    let unit = |p: [f64; 2]| [(p[0] - centre[0]) / spread, (p[1] - centre[1]) / spread];
    let point = |i: usize| unit(raw(i));
    let input = unit(at64);

    // One child per position — the first there — stands for the others.
    let mut stands_for = vec![usize::MAX; n];
    let mut corners: Vec<usize> = Vec::new();
    for &i in &finite {
        let together = corners
            .iter()
            .copied()
            .find(|&c| distance(point(c), point(i)) <= EPSILON.sqrt());
        stands_for[i] = together.unwrap_or(i);
        if together.is_none() {
            corners.push(i);
        }
    }

    let mut raw_weights = vec![0.0f64; n];
    if corners.len() == 1 {
        raw_weights[corners[0]] = 1.0;
    } else if let Some(triangles) = triangulate(&corners, &point) {
        blend_triangles(&triangles, &point, input, &mut raw_weights);
    } else {
        // All on one line: a 1D blend along it, from its first corner
        // towards its farthest.
        let origin = point(corners[0]);
        let far = corners
            .iter()
            .copied()
            .max_by(|&a, &b| distance(origin, point(a)).total_cmp(&distance(origin, point(b))))
            .map_or(origin, &point);
        let direction = [far[0] - origin[0], far[1] - origin[1]];
        let along = |p: [f64; 2]| {
            ((p[0] - origin[0]) * direction[0] + (p[1] - origin[1]) * direction[1]) as f32
        };
        let mut line_weights = vec![0.0f32; corners.len()];
        line::weights(
            corners.len(),
            |k| along(point(corners[k])),
            along(input),
            &mut line_weights,
        );
        for (&corner, &w) in corners.iter().zip(&line_weights) {
            raw_weights[corner] = f64::from(w);
        }
    }

    for &corner in &corners {
        let together = stands_for.iter().filter(|&&s| s == corner).count();
        for (i, w) in out.iter_mut().enumerate().take(n) {
            if stands_for[i] == corner {
                *w = (raw_weights[corner] / together as f64) as f32;
            }
        }
    }
}

/// The Delaunay triangles of `corners`, none overlapping another — where
/// four children lie on one circle, the first of its diagonals in index
/// order. `None` when the corners are all on one line.
fn triangulate(corners: &[usize], point: &impl Fn(usize) -> [f64; 2]) -> Option<Vec<[usize; 3]>> {
    let mut triangles: Vec<[usize; 3]> = Vec::new();
    for (a, &i) in corners.iter().enumerate() {
        for (b, &j) in corners.iter().enumerate().skip(a + 1) {
            for &k in &corners[b + 1..] {
                let (p, q, r) = (point(i), point(j), point(k));
                if cross(p, q, r).abs() <= EPSILON {
                    continue;
                }
                let empty = corners
                    .iter()
                    .filter(|&&m| m != i && m != j && m != k)
                    .all(|&m| !in_circle(p, q, r, point(m)));
                let triangle = [p, q, r];
                let clear = triangles.iter().all(|t| !overlap(triangle, t.map(point)));
                if empty && clear {
                    triangles.push([i, j, k]);
                }
            }
        }
    }
    (!triangles.is_empty()).then_some(triangles)
}

/// The corners of the triangle holding `input`, by barycentric weights; or,
/// outside them all, the two ends of the nearest edge by where along it
/// the input is nearest.
fn blend_triangles(
    triangles: &[[usize; 3]],
    point: &impl Fn(usize) -> [f64; 2],
    input: [f64; 2],
    out: &mut [f64],
) {
    for triangle in triangles {
        let [a, b, c] = triangle.map(point);
        let area = cross(a, b, c);
        let wb = cross(a, input, c) / area;
        let wc = cross(a, b, input) / area;
        let wa = 1.0 - wb - wc;
        let inside = [wa, wb, wc].iter().all(|&w| w >= -EPSILON);
        if inside {
            let weights = [wa, wb, wc].map(|w| w.max(0.0));
            let total: f64 = weights.iter().sum();
            for (&corner, w) in triangle.iter().zip(weights) {
                out[corner] = w / total;
            }
            return;
        }
    }
    let mut nearest: Option<(f64, usize, usize, f64)> = None;
    for triangle in triangles {
        for (from, to) in [(0, 1), (1, 2), (2, 0)] {
            let (i, j) = (triangle[from], triangle[to]);
            let (p, q) = (point(i), point(j));
            let edge = [q[0] - p[0], q[1] - p[1]];
            let length = edge[0] * edge[0] + edge[1] * edge[1];
            let t = (((input[0] - p[0]) * edge[0] + (input[1] - p[1]) * edge[1]) / length)
                .clamp(0.0, 1.0);
            let on = [p[0] + edge[0] * t, p[1] + edge[1] * t];
            let d = distance(on, input);
            if nearest.is_none_or(|(best, ..)| d < best) {
                nearest = Some((d, i, j, t));
            }
        }
    }
    if let Some((_, i, j, t)) = nearest {
        out[i] += 1.0 - t;
        out[j] += t;
    }
}

/// Twice the signed area of `a`, `b`, `c`: positive when anticlockwise.
fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Whether `d` is strictly inside the circle through `a`, `b`, `c` — on it
/// is not, so four children on one circle leave both diagonals' triangles.
fn in_circle(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let row = |p: [f64; 2]| {
        let (x, y) = (p[0] - d[0], p[1] - d[1]);
        [x, y, x * x + y * y]
    };
    let [a, b, c] = [row(a), row(b), row(c)];
    let det = a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
        + a[2] * (b[0] * c[1] - b[1] * c[0]);
    let orientation = cross([a[0], a[1]], [b[0], b[1]], [c[0], c[1]]).signum();
    det * orientation > EPSILON
}

/// Whether two triangles' insides overlap; sharing an edge or a corner is
/// not overlapping. Separating axes: the normals of their six edges.
fn overlap(a: [[f64; 2]; 3], b: [[f64; 2]; 3]) -> bool {
    let separated = |axis: [f64; 2]| {
        let span = |t: [[f64; 2]; 3]| {
            let d = t.map(|p| p[0] * axis[0] + p[1] * axis[1]);
            (d[0].min(d[1]).min(d[2]), d[0].max(d[1]).max(d[2]))
        };
        let ((a_low, a_high), (b_low, b_high)) = (span(a), span(b));
        a_high <= b_low + EPSILON || b_high <= a_low + EPSILON
    };
    let normals = |t: [[f64; 2]; 3]| {
        [(0, 1), (1, 2), (2, 0)].map(|(i, j)| [t[i][1] - t[j][1], t[j][0] - t[i][0]])
    };
    !normals(a).into_iter().chain(normals(b)).any(separated)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(positions: &[[f32; 2]], input: [f32; 2]) -> Vec<f32> {
        let mut out = vec![f32::NAN; positions.len()];
        weights(positions.len(), |i| positions[i], input, &mut out);
        out
    }

    fn close(a: &[f32], b: &[f32]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-5)
    }

    /// Idle in the middle, a walk each way: a directional blend's layout.
    const CROSS: [[f32; 2]; 5] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]];

    #[test]
    fn at_a_child_it_alone_plays() {
        for (i, &p) in CROSS.iter().enumerate() {
            let mut expected = [0.0; 5];
            expected[i] = 1.0;
            assert!(close(&at(&CROSS, p), &expected), "{i}: {:?}", at(&CROSS, p));
        }
    }

    #[test]
    fn on_an_edge_its_two_ends_mix() {
        assert!(close(&at(&CROSS, [0.5, 0.0]), &[0.5, 0.5, 0.0, 0.0, 0.0]));
        assert!(
            close(&at(&CROSS, [0.5, 0.5]), &[0.0, 0.5, 0.5, 0.0, 0.0]),
            "{:?}",
            at(&CROSS, [0.5, 0.5])
        );
    }

    #[test]
    fn in_a_triangle_its_corners_mix_by_barycentric_weights() {
        let w = at(&CROSS, [0.25, 0.25]);
        assert!(close(&w, &[0.5, 0.25, 0.25, 0.0, 0.0]), "{w:?}");
        let w = at(&CROSS, [-0.2, -0.6]);
        assert!(close(&w, &[0.2, 0.0, 0.0, 0.2, 0.6]), "{w:?}");
    }

    #[test]
    fn outside_the_hull_the_nearest_edge_mixes() {
        let w = at(&CROSS, [2.0, 2.0]);
        assert!(close(&w, &[0.0, 0.5, 0.5, 0.0, 0.0]), "{w:?}");
        let w = at(&CROSS, [3.0, 0.0]);
        assert!(close(&w, &[0.0, 1.0, 0.0, 0.0, 0.0]), "a corner: {w:?}");
        let triangle = [[0.0, 0.0], [2.0, 0.0], [0.0, 2.0]];
        let w = at(&triangle, [1.0, -5.0]);
        assert!(close(&w, &[0.5, 0.5, 0.0]), "{w:?}");
    }

    #[test]
    fn a_square_on_one_circle_is_split_once_and_blends_continuously() {
        let square = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
        let w = at(&square, [0.0, 0.0]);
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-5, "{w:?}");
        // Either side of the diagonal it was split by, the same weights.
        let (a, b) = (at(&square, [0.3, 0.3001]), at(&square, [0.3001, 0.3]));
        assert!(
            a.iter().zip(&b).all(|(a, b)| (a - b).abs() < 1e-3),
            "{a:?} {b:?}"
        );
    }

    #[test]
    fn one_child_a_line_and_shared_positions_clamp_sensibly() {
        assert!(close(&at(&[[3.0, 4.0]], [-9.0, 9.0]), &[1.0]));
        let line = [[0.0, 0.0], [2.0, 2.0], [1.0, 1.0]];
        assert!(close(&at(&line, [1.5, 1.5]), &[0.0, 0.5, 0.5]));
        assert!(close(&at(&line, [-4.0, 0.0]), &[1.0, 0.0, 0.0]), "clamped");
        let shared = [[0.0, 0.0], [1.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
        let w = at(&shared, [1.0, 0.0]);
        assert!(close(&w, &[0.0, 0.5, 0.5, 0.0]), "{w:?}");
        let all_one = [[2.0, 2.0], [2.0, 2.0]];
        assert!(close(&at(&all_one, [0.0, 0.0]), &[0.5, 0.5]));
    }

    #[test]
    fn numbers_that_are_not_finite_never_make_a_nan() {
        let w = at(&[[f32::NAN, 0.0], [1.0, 0.0], [0.0, 1.0]], [0.2, 0.2]);
        assert!(
            close(&w, &[0.0, 0.5, 0.5]),
            "the two left are a line: {w:?}"
        );
        let w = at(&CROSS, [f32::INFINITY, f32::NAN]);
        assert!(
            close(&w, &[1.0, 0.0, 0.0, 0.0, 0.0]),
            "read as the origin: {w:?}"
        );
        assert!(at(&[[f32::NAN, f32::NAN]], [0.0, 0.0]) == [0.0]);
        assert!(at(&[], [0.0, 0.0]).is_empty());
        let huge = at(&[[-1e30, 0.0], [1e30, 0.0], [0.0, 1e30]], [5.0, 5.0]);
        assert!(huge.iter().all(|w| w.is_finite()), "{huge:?}");
    }
}
