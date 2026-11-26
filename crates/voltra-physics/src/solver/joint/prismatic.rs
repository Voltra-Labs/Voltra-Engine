//! A prismatic joint: the mover's anchor kept on a line along the base's
//! axis and its angle held, free only to slide — with a spring, a motor and
//! limits along the line (Box2D's `b2SolvePrismaticJoint`).

use voltra_render::glam::Vec2;
use voltra_scene::JointKind;

use super::line::Line;
use super::{slide, JointConstraint, Side};
use crate::solver::Softness;

/// One solve, in Box2D's order: the spring and the motor, the limits, then
/// the line and the angle together — solved last, so they win.
pub(super) fn solve(
    joint: &mut JointConstraint,
    a: &mut Side,
    b: &mut Side,
    softness: Softness,
    h: f32,
    use_bias: bool,
) {
    let JointKind::Prismatic {
        reference_angle,
        limit,
        motor,
        spring,
        ..
    } = joint.kind
    else {
        return;
    };
    let line = Line::now(joint, a, b);
    if let Some(motor) = motor {
        let impulse = line.axial_mass * (motor.speed - line.along_speed(a, b));
        let max = h * motor.max_force.max(0.0);
        let old = joint.impulses.motor;
        joint.impulses.motor = (old + impulse).clamp(-max, max);
        line.push_along(a, b, joint.impulses.motor - old);
    }
    slide::solve(joint, a, b, &line, (spring, limit), softness, h, use_bias);

    // Across the line and about the joint at once: a 2×2 block, as Box2D
    // solves it, since a push across the line turns the bodies too.
    let cdot = Vec2::new(line.across_speed(a, b), a.w - b.w);
    let (mut bias, mut mass_scale, mut impulse_scale) = (Vec2::ZERO, 1.0, 0.0);
    if use_bias {
        let c = Vec2::new(line.across, joint.angle_now(a, b, reference_angle));
        bias = c * softness.bias_rate;
        mass_scale = softness.mass_scale;
        impulse_scale = softness.impulse_scale;
    }
    let k11 = a.m + b.m + b.i * line.s1 * line.s1 + a.i * line.s2 * line.s2;
    let k12 = b.i * line.s1 + a.i * line.s2;
    // Neither side turning: the angle needs nothing, and a unit there keeps
    // the block solvable.
    let turning = a.i + b.i;
    let k22 = if turning > 0.0 { turning } else { 1.0 };
    let det = k11 * k22 - k12 * k12;
    if det.abs() <= f32::EPSILON {
        return;
    }
    let rhs = cdot + bias;
    let solved = Vec2::new(k22 * rhs.x - k12 * rhs.y, k11 * rhs.y - k12 * rhs.x) / det;
    let held = Vec2::new(joint.impulses.perp, joint.impulses.angular);
    let impulse = -mass_scale * solved - impulse_scale * held;
    joint.impulses.perp += impulse.x;
    joint.impulses.angular += impulse.y;
    line.push_across(a, b, impulse.x, impulse.y);
}
