//! The sweep for every pair with a capsule or a polygon in it: conservative
//! advancement over GJK.
//!
//! Measure the gap, move the shape as far as the gap allows at the speed it
//! closes, measure again. Under a translation the gap between two convex
//! shapes is a convex function of time, so a step never overshoots, a shape
//! whose gap is not closing never will, and the walk ends within the touch
//! tolerance of first contact — Box2D's time of impact, without rotation.

use voltra_render::glam::Vec2;

use super::{PARALLEL, TOUCH};
use crate::geometry::{closest, Hull};

/// Enough for a grazing approach to a round corner, which closes the gap
/// geometrically rather than in one step; a face is met in one or two.
const MAX_ITERATIONS: usize = 32;

/// When, as a fraction of `motion`, `a` first touches `b`, and the normal
/// pushing `a` away there: [`super::sweep::sweep`]'s answer, for any hulls.
pub(super) fn advance(a: &Hull, motion: Vec2, b: &Hull) -> Option<(f32, Vec2)> {
    let radius = a.radius + b.radius;
    let still = motion.length() * PARALLEL;
    let mut t = 0.0_f32;
    let mut normal = Vec2::ZERO;
    for _ in 0..MAX_ITERATIONS {
        let moved = a.moved(motion * t);
        let at = closest(moved.vertices(), b.vertices());
        let Some(towards) = at.face_normal(moved.vertices(), b.vertices()) else {
            // The cores meet: overlapping from the start, which the narrow
            // phase measures; later, the step before was already a touch.
            return Some(if t == 0.0 {
                (-1.0, Vec2::ZERO)
            } else {
                (t, normal)
            });
        };
        normal = towards;
        let gap = at.distance - radius;
        if t == 0.0 && gap < -TOUCH {
            return Some((-1.0, normal));
        }
        // How fast the gap closes, per whole motion.
        let closing = -motion.dot(normal);
        if closing <= still {
            // Moving along the touch or away from it, and convexity says it
            // never closes after.
            return None;
        }
        if gap <= TOUCH {
            return Some((t, normal));
        }
        t += (gap - 0.5 * TOUCH) / closing;
        if t > 1.0 {
            return None;
        }
    }
    Some((t, normal))
}
