//! A symmetric 2×2 effective mass solved: what the point constraint and a
//! prismatic's line and angle both need (Box2D's `b2Solve22`).

use voltra_render::glam::Vec2;

/// `x` with `K x = rhs`, `K = [[k11, k12], [k12, k22]]`; `None` when `K` is
/// singular — neither side can move along some direction.
///
/// Singular is judged against `K`'s own scale: `det` goes as the inverse
/// mass squared, so a fixed threshold would take a body of a few tonnes,
/// whose inverse mass is a few thousandths, for one that cannot move at all.
pub(super) fn solve(k11: f32, k12: f32, k22: f32, rhs: Vec2) -> Option<Vec2> {
    let det = k11 * k22 - k12 * k12;
    // `K` is positive semi-definite: `det` at most `k11 · k22`, and at or
    // below rounding of that only when it is singular. Not a number is no
    // solution either.
    if det.is_nan() || det <= f32::EPSILON * k11 * k22 {
        return None;
    }
    Some(Vec2::new(k22 * rhs.x - k12 * rhs.y, k11 * rhs.y - k12 * rhs.x) / det)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heavy_body_is_solved_and_an_immovable_pair_is_not() {
        // Ten tonnes against the world: an inverse mass of 1e-4 each way.
        let heavy = solve(1e-4, 0.0, 1e-4, Vec2::new(1.0, 2.0)).expect("solvable");
        assert!((heavy - Vec2::new(1e4, 2e4)).length() < 1.0, "{heavy}");
        assert!(solve(0.0, 0.0, 0.0, Vec2::ONE).is_none());
        assert!(solve(1.0, 1.0, 1.0, Vec2::ONE).is_none(), "rank one");
        assert!(solve(f32::NAN, 0.0, 1.0, Vec2::ONE).is_none());
    }
}
