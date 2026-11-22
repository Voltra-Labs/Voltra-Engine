//! A weld joint: the anchors held together and the angle kept (Box2D's
//! `b2SolveWeldJoint`, rigid).

use voltra_scene::JointKind;

use super::{point, JointConstraint, Side};
use crate::solver::Softness;

/// One solve: the angle, then the anchors.
pub(super) fn solve(
    joint: &mut JointConstraint,
    a: &mut Side,
    b: &mut Side,
    softness: Softness,
    use_bias: bool,
) {
    let turning = a.i + b.i;
    if turning > 0.0 {
        let reference = match joint.kind {
            JointKind::Weld { reference_angle } => reference_angle,
            _ => 0.0,
        };
        let (mut bias, mut mass_scale, mut impulse_scale) = (0.0, 1.0, 0.0);
        if use_bias {
            bias = joint.angle_now(a, b, reference) * softness.bias_rate;
            mass_scale = softness.mass_scale;
            impulse_scale = softness.impulse_scale;
        }
        let cdot = a.w - b.w;
        let impulse =
            -mass_scale * (cdot + bias) / turning - impulse_scale * joint.impulses.angular;
        joint.impulses.angular += impulse;
        a.w += a.i * impulse;
        b.w -= b.i * impulse;
    }
    point::solve(joint, a, b, softness, use_bias);
}
