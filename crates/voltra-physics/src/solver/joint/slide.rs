//! Along a line: the spring and the limits a wheel and a prismatic joint
//! share (Box2D's `b2SolveWheelJoint` and `b2SolvePrismaticJoint` solve
//! them alike).

use voltra_scene::{RestSpring, TranslationLimit};

use super::line::Line;
use super::revolute::limit_terms;
use super::{JointConstraint, Side};
use crate::solver::Softness;

/// The spring and the limits along `line`'s axis. The spring acts in the
/// relax pass too, as every joint spring does; the limits are speculative
/// inside and soft past them.
#[allow(clippy::too_many_arguments)]
pub(super) fn solve(
    joint: &mut JointConstraint,
    a: &mut Side,
    b: &mut Side,
    line: &Line,
    (spring, limit): (Option<RestSpring>, Option<TranslationLimit>),
    softness: Softness,
    h: f32,
    use_bias: bool,
) {
    if let Some(spring) = spring.filter(|s| s.hertz > 0.0) {
        let s = joint.spring;
        let c = line.translation - spring.rest;
        let impulse = -s.mass_scale * line.axial_mass * (line.along_speed(a, b) + s.bias_rate * c)
            - s.impulse_scale * joint.impulses.spring;
        joint.impulses.spring += impulse;
        line.push_along(a, b, impulse);
    }
    let Some(limit) = limit else {
        return;
    };
    // Lower: at least `lower` along the axis.
    {
        let (bias, mass_scale, impulse_scale) =
            limit_terms(line.translation - limit.lower, softness, h, use_bias);
        let impulse = -line.axial_mass * mass_scale * (line.along_speed(a, b) + bias)
            - impulse_scale * joint.impulses.lower;
        let new = (joint.impulses.lower + impulse).max(0.0);
        let impulse = new - joint.impulses.lower;
        joint.impulses.lower = new;
        line.push_along(a, b, impulse);
    }
    // Upper: at most `upper`.
    {
        let (bias, mass_scale, impulse_scale) =
            limit_terms(limit.upper - line.translation, softness, h, use_bias);
        let impulse = -line.axial_mass * mass_scale * (-line.along_speed(a, b) + bias)
            - impulse_scale * joint.impulses.upper;
        let new = (joint.impulses.upper + impulse).max(0.0);
        let impulse = new - joint.impulses.upper;
        joint.impulses.upper = new;
        line.push_along(a, b, -impulse);
    }
}
