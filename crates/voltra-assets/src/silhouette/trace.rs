//! Tracing the outline of an image's opaque texels.
//!
//! Every side between an opaque texel and a transparent one (or the edge of
//! the rectangle) becomes a directed edge with the opaque texel on its
//! right, in image coordinates (y down); the edges link up into closed
//! loops. Loops around opaque islands wind one way and loops around holes
//! the other: only the outer ones are kept — a shadow is cast by a shape's
//! outline, and a hole's edges would shadow its own inside. Each loop is
//! then simplified (Douglas–Peucker) so a staircase of texels becomes the
//! slope it draws.

use std::collections::HashMap;

use voltra_render::glam::Vec2;

/// How opaque a texel must be to cast unless a caster says otherwise: half.
pub const ALPHA_THRESHOLD: u8 = 128;

/// How far a simplified outline may stray from the texel edges, in texels.
/// Under one keeps the shape; more would round off thin parts.
pub const TOLERANCE: f32 = 0.75;

/// The outer outlines of the texels of `rgba` (`width` × `height`, RGBA8)
/// inside `rect` (x, y, width, height) whose alpha reaches `threshold`
/// ([`ALPHA_THRESHOLD`] by default), each a closed loop of points in `0..=1`
/// across the rectangle (u right, v down, as a quad's UVs run). Empty for a
/// rectangle that lies outside the image or holds nothing opaque.
pub fn trace(
    rgba: &[u8],
    width: u32,
    height: u32,
    rect: [u32; 4],
    threshold: u8,
) -> Vec<Vec<Vec2>> {
    let [x0, y0, w, h] = rect;
    if w == 0
        || h == 0
        || x0.saturating_add(w) > width
        || y0.saturating_add(h) > height
        || rgba.len() < (width as usize) * (height as usize) * 4
    {
        return Vec::new();
    }
    let solid = |x: i32, y: i32| {
        if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
            return false;
        }
        let index = ((y0 as usize + y as usize) * width as usize + x0 as usize + x as usize) * 4;
        rgba[index + 3] >= threshold
    };

    // Directed edges, from corner to corner, opaque texel on the right.
    let mut next: HashMap<(i32, i32), Vec<(i32, i32)>> = HashMap::new();
    let mut edge = |from: (i32, i32), to: (i32, i32)| next.entry(from).or_default().push(to);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            if !solid(x, y) {
                continue;
            }
            if !solid(x, y - 1) {
                edge((x, y), (x + 1, y));
            }
            if !solid(x + 1, y) {
                edge((x + 1, y), (x + 1, y + 1));
            }
            if !solid(x, y + 1) {
                edge((x + 1, y + 1), (x, y + 1));
            }
            if !solid(x - 1, y) {
                edge((x, y + 1), (x, y));
            }
        }
    }

    let mut loops = Vec::new();
    let mut starts: Vec<(i32, i32)> = next.keys().copied().collect();
    // Deterministic, so the same image always traces the same outline.
    starts.sort_unstable();
    for start in starts {
        while next.get(&start).is_some_and(|ends| !ends.is_empty()) {
            let mut ring = vec![start];
            let mut at = start;
            let mut from = start;
            while let Some(to) = next.get_mut(&at).and_then(|ends| take_turn(ends, from, at)) {
                if to == start {
                    break;
                }
                ring.push(to);
                from = at;
                at = to;
            }
            if ring.len() >= 3 && twice_area(&ring) > 0 {
                loops.push(ring);
            }
        }
    }

    let scale = Vec2::new(w as f32, h as f32);
    loops
        .into_iter()
        .map(|ring| {
            let points: Vec<Vec2> = ring
                .into_iter()
                .map(|(x, y)| Vec2::new(x as f32, y as f32))
                .collect();
            // A shape smaller than the tolerance would simplify away;
            // it keeps its texel edges instead.
            let simple = simplify_closed(&points, TOLERANCE);
            let kept = if simple.len() >= 3 { simple } else { points };
            kept.into_iter().map(|p| p / scale).collect::<Vec<_>>()
        })
        .filter(|ring| ring.len() >= 3)
        .collect()
}

/// The next corner out of `at`, arriving from `from`, removed from `ends`:
/// the rightmost turn there is — right, then straight, then left, opaque
/// side on the right. Where two islands touch only at a corner (or a hole
/// reaches the outside diagonally) a corner has two ways out, and always
/// turning right keeps each loop around one island rather than walking a
/// figure-eight through both. The first edge of a loop has no arrival and
/// takes any.
fn take_turn(ends: &mut Vec<(i32, i32)>, from: (i32, i32), at: (i32, i32)) -> Option<(i32, i32)> {
    if ends.len() <= 1 || from == at {
        return ends.pop();
    }
    let (dx, dy) = (at.0 - from.0, at.1 - from.1);
    // Image coordinates, y down: a right turn of (dx, dy) is (-dy, dx).
    let order = [(-dy, dx), (dx, dy), (dy, -dx)];
    let index = order
        .iter()
        .find_map(|d| ends.iter().position(|e| (e.0 - at.0, e.1 - at.1) == *d))
        .unwrap_or(ends.len() - 1);
    Some(ends.remove(index))
}

/// Twice the signed area in image coordinates: positive for a loop around
/// opaque texels (they are on its right, y down), negative around a hole.
fn twice_area(ring: &[(i32, i32)]) -> i64 {
    ring.iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| i64::from(a.0) * i64::from(b.1) - i64::from(b.0) * i64::from(a.1))
        .sum()
}

/// Douglas–Peucker on a closed loop: split at the point farthest from the
/// first, simplify both halves as open lines, join.
fn simplify_closed(points: &[Vec2], tolerance: f32) -> Vec<Vec2> {
    if points.len() <= 3 {
        return points.to_vec();
    }
    let far = (1..points.len())
        .max_by(|&a, &b| {
            points[a]
                .distance_squared(points[0])
                .total_cmp(&points[b].distance_squared(points[0]))
        })
        .unwrap_or(1);
    let mut first = simplify_open(&points[..=far], tolerance);
    let mut second: Vec<Vec2> = points[far..].to_vec();
    second.push(points[0]);
    let second = simplify_open(&second, tolerance);
    first.pop();
    first.extend_from_slice(&second[..second.len() - 1]);
    first
}

fn simplify_open(points: &[Vec2], tolerance: f32) -> Vec<Vec2> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let (a, b) = (points[0], points[points.len() - 1]);
    let (index, distance) = points[1..points.len() - 1]
        .iter()
        .enumerate()
        .map(|(i, p)| (i + 1, distance_to_line(*p, a, b)))
        .max_by(|x, y| x.1.total_cmp(&y.1))
        .unwrap_or((0, 0.0));
    if distance <= tolerance {
        return vec![a, b];
    }
    let mut left = simplify_open(&points[..=index], tolerance);
    let right = simplify_open(&points[index..], tolerance);
    left.pop();
    left.extend(right);
    left
}

fn distance_to_line(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let along = b - a;
    let length = along.length();
    if length == 0.0 {
        return p.distance(a);
    }
    (along.perp_dot(p - a) / length).abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `size` × `size` image, opaque where `opaque(x, y)`.
    fn image(size: u32, opaque: impl Fn(u32, u32) -> bool) -> Vec<u8> {
        (0..size * size)
            .flat_map(|i| {
                let (x, y) = (i % size, i / size);
                [255, 255, 255, if opaque(x, y) { 255 } else { 0 }]
            })
            .collect()
    }

    fn bounds(ring: &[Vec2]) -> (Vec2, Vec2) {
        ring.iter().fold(
            (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        )
    }

    #[test]
    fn a_square_traces_as_its_four_corners() {
        let rgba = image(8, |x, y| (2..6).contains(&x) && (2..6).contains(&y));
        let loops = trace(&rgba, 8, 8, [0, 0, 8, 8], ALPHA_THRESHOLD);
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].len(), 4, "{:?}", loops[0]);
        assert_eq!(bounds(&loops[0]), (Vec2::splat(0.25), Vec2::splat(0.75)));
    }

    #[test]
    fn a_staircase_simplifies_to_its_slope() {
        // A right triangle drawn in texels: below the diagonal.
        let rgba = image(16, |x, y| x <= y);
        let loops = trace(&rgba, 16, 16, [0, 0, 16, 16], ALPHA_THRESHOLD);
        assert_eq!(loops.len(), 1);
        assert!(
            loops[0].len() <= 5,
            "far fewer than the 48 steps: {:?}",
            loops[0]
        );
    }

    #[test]
    fn separate_islands_trace_separately_and_holes_are_left_out() {
        // A ring (with a hole) and a dot.
        let rgba = image(16, |x, y| {
            let ring = (1..9).contains(&x) && (1..9).contains(&y);
            let hole = (3..7).contains(&x) && (3..7).contains(&y);
            (ring && !hole) || (x == 12 && y == 12)
        });
        let loops = trace(&rgba, 16, 16, [0, 0, 16, 16], ALPHA_THRESHOLD);
        assert_eq!(loops.len(), 2, "the ring's outside and the dot: {loops:?}");
    }

    #[test]
    fn a_rectangle_traces_only_its_own_texels_in_its_own_units() {
        // Two cells side by side; the right one is opaque in its left half.
        let rgba = image(8, |x, _| (4..6).contains(&x));
        let loops = trace(&rgba, 8, 8, [4, 0, 4, 8], ALPHA_THRESHOLD);
        assert_eq!(loops.len(), 1);
        assert_eq!(bounds(&loops[0]), (Vec2::ZERO, Vec2::new(0.5, 1.0)));
    }

    #[test]
    fn nothing_opaque_or_a_rectangle_off_the_image_traces_nothing() {
        let clear = image(4, |_, _| false);
        assert!(trace(&clear, 4, 4, [0, 0, 4, 4], ALPHA_THRESHOLD).is_empty());
        let full = image(4, |_, _| true);
        assert!(
            trace(&full, 4, 4, [2, 2, 4, 4], ALPHA_THRESHOLD).is_empty(),
            "off the image"
        );
        assert!(
            trace(&full, 4, 4, [0, 0, 0, 4], ALPHA_THRESHOLD).is_empty(),
            "no width"
        );
        assert!(
            trace(&full[..8], 4, 4, [0, 0, 4, 4], ALPHA_THRESHOLD).is_empty(),
            "short buffer"
        );
    }

    #[test]
    fn islands_touching_only_at_a_corner_trace_as_two_loops() {
        // A 2x2 checkerboard: top-left and bottom-right opaque.
        let rgba = image(2, |x, y| x == y);
        let loops = trace(&rgba, 2, 2, [0, 0, 2, 2], ALPHA_THRESHOLD);
        assert_eq!(loops.len(), 2, "{loops:?}");
        assert!(loops.iter().all(|ring| ring.len() == 4));
    }

    #[test]
    fn a_hole_that_reaches_the_outside_at_a_corner_is_still_left_out() {
        // A 3x3 block with its centre clear and its corner texel clear: the
        // hole touches the outside only diagonally.
        let rgba = image(3, |x, y| !((x == 1 && y == 1) || (x == 2 && y == 2)));
        let loops = trace(&rgba, 3, 3, [0, 0, 3, 3], ALPHA_THRESHOLD);
        assert_eq!(loops.len(), 1, "the outside only: {loops:?}");
    }

    #[test]
    fn the_threshold_decides_what_casts() {
        // Alpha 100: under the default, over a low threshold.
        let rgba: Vec<u8> = (0..16).flat_map(|_| [0, 0, 0, 100]).collect();
        assert!(trace(&rgba, 4, 4, [0, 0, 4, 4], ALPHA_THRESHOLD).is_empty());
        assert_eq!(trace(&rgba, 4, 4, [0, 0, 4, 4], 64).len(), 1);
    }
}
