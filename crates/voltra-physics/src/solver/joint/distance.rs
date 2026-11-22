//! A distance joint: a rod, a spring, or a range — a rope at its loosest
//! (Box2D's `b2SolveDistanceJoint`).

use voltra_render::glam::Vec2;
use voltra_scene::JointKind;

use super::revolute::limit_terms;
use super::{spin, JointConstraint, Side};
use crate::solver::Softness;

/// One solve. With `min` below `max`: the spring, if any, then the two
/// limits; otherwise the distance held at `length` — by the spring if there
/// is one, rigidly if not.
pub(super) fn solve(
    joint: &mut JointConstraint,
    a: &mut Side,
    b: &mut Side,
    softness: Softness,
    h: f32,
    use_bias: bool,
) {
    let JointKind::Distance {
        length,
        min,
        max,
        spring,
    } = joint.kind
    else {
        return;
    };
    let (ra, rb) = joint.arms(a, b);
    let d = (b.dp - a.dp) + joint.delta_center + rb - ra;
    let now = d.length();
    let Some(axis) = d.try_normalize() else {
        // The anchors on one point: no direction to hold them apart along.
        return;
    };
    let (cra, crb) = (ra.perp_dot(axis), rb.perp_dot(axis));
    let k = a.m + b.m + a.i * cra * cra + b.i * crb * crb;
    if k <= 0.0 {
        return;
    }
    let axial_mass = 1.0 / k;
    let along = |a: &Side, b: &Side| (b.v + spin(b.w, rb) - a.v - spin(a.w, ra)).dot(axis);
    let push = |a: &mut Side, b: &mut Side, impulse: f32| {
        let p: Vec2 = axis * impulse;
        a.v -= a.m * p;
        a.w -= a.i * ra.perp_dot(p);
        b.v += b.m * p;
        b.w += b.i * rb.perp_dot(p);
    };

    if min < max {
        if spring.is_some_and(|s| s.hertz > 0.0) {
            let s = joint.spring;
            let c = now - length;
            let bias = s.bias_rate * c;
            let impulse = -s.mass_scale * axial_mass * (along(a, b) + bias)
                - s.impulse_scale * joint.impulses.axial;
            joint.impulses.axial += impulse;
            push(a, b, impulse);
        }
        // At least `min` apart.
        {
            let (bias, mass_scale, impulse_scale) = limit_terms(now - min, softness, h, use_bias);
            let impulse = -mass_scale * axial_mass * (along(a, b) + bias)
                - impulse_scale * joint.impulses.lower;
            let new = (joint.impulses.lower + impulse).max(0.0);
            let impulse = new - joint.impulses.lower;
            joint.impulses.lower = new;
            push(a, b, impulse);
        }
        // At most `max`.
        {
            let (bias, mass_scale, impulse_scale) = limit_terms(max - now, softness, h, use_bias);
            let impulse = -mass_scale * axial_mass * (-along(a, b) + bias)
                - impulse_scale * joint.impulses.upper;
            let new = (joint.impulses.upper + impulse).max(0.0);
            let impulse = new - joint.impulses.upper;
            joint.impulses.upper = new;
            push(a, b, -impulse);
        }
    } else {
        let held = match spring {
            Some(s) if s.hertz > 0.0 => joint.spring,
            _ if use_bias => softness,
            _ => Softness::RIGID,
        };
        let c = now - length;
        let bias = if spring.is_some_and(|s| s.hertz > 0.0) || use_bias {
            held.bias_rate * c
        } else {
            0.0
        };
        let impulse = -held.mass_scale * axial_mass * (along(a, b) + bias)
            - held.impulse_scale * joint.impulses.axial;
        joint.impulses.axial += impulse;
        push(a, b, impulse);
    }
}
