//! The two anchors held together: the part a revolute and a weld joint share
//! (Box2D's point constraint).

use voltra_render::glam::Vec2;

use super::{spin, JointConstraint, Side};
use crate::solver::Softness;

/// One solve of the anchors' constraint, `softness` the joints' spring.
pub(super) fn solve(
    joint: &mut JointConstraint,
    a: &mut Side,
    b: &mut Side,
    softness: Softness,
    use_bias: bool,
) {
    let (ra, rb) = joint.arms(a, b);
    let cdot = b.v + spin(b.w, rb) - a.v - spin(a.w, ra);

    let (mut bias, mut mass_scale, mut impulse_scale) = (Vec2::ZERO, 1.0, 0.0);
    if use_bias {
        let separation = (b.dp - a.dp) + (rb - ra) + joint.delta_center;
        bias = separation * softness.bias_rate;
        mass_scale = softness.mass_scale;
        impulse_scale = softness.impulse_scale;
    }

    let k11 = a.m + b.m + ra.y * ra.y * a.i + rb.y * rb.y * b.i;
    let k12 = -ra.y * ra.x * a.i - rb.y * rb.x * b.i;
    let k22 = a.m + b.m + ra.x * ra.x * a.i + rb.x * rb.x * b.i;
    let det = k11 * k22 - k12 * k12;
    if det.abs() <= f32::EPSILON {
        // Neither side can move: nothing to solve.
        return;
    }
    let rhs = cdot + bias;
    let solved = Vec2::new(k22 * rhs.x - k12 * rhs.y, k11 * rhs.y - k12 * rhs.x) / det;
    let impulse = -mass_scale * solved - impulse_scale * joint.impulses.linear;
    joint.impulses.linear += impulse;

    a.v -= a.m * impulse;
    a.w -= a.i * ra.perp_dot(impulse);
    b.v += b.m * impulse;
    b.w += b.i * rb.perp_dot(impulse);
}
