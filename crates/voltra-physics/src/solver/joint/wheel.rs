//! A wheel joint: the wheel's anchor kept on a line along the chassis's
//! axis, sprung along it and free to turn, with an optional motor (Box2D's
//! `b2SolveWheelJoint`).

use voltra_scene::JointKind;

use super::line::Line;
use super::{slide, JointConstraint, Side};
use crate::solver::Softness;

/// One solve, in Box2D's order: the motor, the suspension, the limits, then
/// the line — solved last, so it wins.
pub(super) fn solve(
    joint: &mut JointConstraint,
    a: &mut Side,
    b: &mut Side,
    softness: Softness,
    h: f32,
    use_bias: bool,
) {
    let JointKind::Wheel {
        spring,
        limit,
        motor,
        ..
    } = joint.kind
    else {
        return;
    };
    // The motor turns the wheel against the chassis, as a revolute's does.
    let turning = a.i + b.i;
    if let Some(motor) = motor.filter(|_| turning > 0.0) {
        let cdot = a.w - b.w - motor.speed;
        let impulse = -cdot / turning;
        let max = h * motor.max_torque.max(0.0);
        let old = joint.impulses.motor;
        joint.impulses.motor = (old + impulse).clamp(-max, max);
        let impulse = joint.impulses.motor - old;
        a.w += a.i * impulse;
        b.w -= b.i * impulse;
    }
    // Where the bodies are does not change within a solve, only how fast
    // they move: the line is worked out once.
    let line = Line::now(joint, a, b);
    slide::spring(joint, a, b, &line, spring);
    slide::limits(joint, a, b, &line, limit, softness, h, use_bias);

    let (mut bias, mut mass_scale, mut impulse_scale) = (0.0, 1.0, 0.0);
    if use_bias {
        bias = softness.bias_rate * line.across;
        mass_scale = softness.mass_scale;
        impulse_scale = softness.impulse_scale;
    }
    let impulse = -mass_scale * line.perp_mass(a, b) * (line.across_speed(a, b) + bias)
        - impulse_scale * joint.impulses.perp;
    joint.impulses.perp += impulse;
    line.push_across(a, b, impulse, 0.0);
}
