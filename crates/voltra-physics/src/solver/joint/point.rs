//! The two anchors held together: the part a revolute and a weld joint share
//! (Box2D's point constraint), and — soft and capped — a mouse joint's pull.

use voltra_render::glam::Vec2;

use super::{block, spin, JointConstraint, Side};
use crate::solver::Softness;

/// One solve of the anchors' constraint, `softness` the joints' spring.
pub(super) fn solve(
    joint: &mut JointConstraint,
    a: &mut Side,
    b: &mut Side,
    softness: Softness,
    use_bias: bool,
) {
    hold(joint, a, b, use_bias.then_some(softness), f32::INFINITY);
}

/// The anchors pulled together by `soft` — or, with `None`, only their
/// relative speed taken out, the relax pass's rigid solve — with the
/// accumulated impulse no longer than `max_impulse`.
pub(super) fn hold(
    joint: &mut JointConstraint,
    a: &mut Side,
    b: &mut Side,
    soft: Option<Softness>,
    max_impulse: f32,
) {
    let (ra, rb) = joint.arms(a, b);
    let cdot = b.v + spin(b.w, rb) - a.v - spin(a.w, ra);

    let (mut bias, mut mass_scale, mut impulse_scale) = (Vec2::ZERO, 1.0, 0.0);
    if let Some(soft) = soft {
        let separation = (b.dp - a.dp) + (rb - ra) + joint.delta_center;
        bias = separation * soft.bias_rate;
        mass_scale = soft.mass_scale;
        impulse_scale = soft.impulse_scale;
    }

    let k11 = a.m + b.m + ra.y * ra.y * a.i + rb.y * rb.y * b.i;
    let k12 = -ra.y * ra.x * a.i - rb.y * rb.x * b.i;
    let k22 = a.m + b.m + ra.x * ra.x * a.i + rb.x * rb.x * b.i;
    let Some(solved) = block::solve(k11, k12, k22, cdot + bias) else {
        // Neither side can move: nothing to solve.
        return;
    };
    let old = joint.impulses.linear;
    let mut total = old - mass_scale * solved - impulse_scale * old;
    // `max_impulse` infinite never caps: no length is past it.
    if total.length() > max_impulse {
        total = total.normalize_or_zero() * max_impulse;
    }
    joint.impulses.linear = total;
    let impulse = total - old;

    a.v -= a.m * impulse;
    a.w -= a.i * ra.perp_dot(impulse);
    b.v += b.m * impulse;
    b.w += b.i * rb.perp_dot(impulse);
}
