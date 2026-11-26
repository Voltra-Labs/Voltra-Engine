//! A revolute joint: the anchors held together, the angle free but for a
//! spring, a motor and a limit (Box2D's `b2SolveRevoluteJoint`).

use voltra_scene::{AngleLimit, JointKind, Motor, RestSpring};

use super::{point, JointConstraint, Side};
use crate::solver::Softness;

/// The revolute's optional parts, as its kind holds them.
pub(super) struct Parts {
    pub(super) limit: Option<AngleLimit>,
    pub(super) motor: Option<Motor>,
    pub(super) spring: Option<RestSpring>,
}

/// One solve: the spring, the motor, then the limit, then the anchors —
/// Box2D's order, so the anchors, solved last, win.
pub(super) fn solve(
    joint: &mut JointConstraint,
    a: &mut Side,
    b: &mut Side,
    Parts {
        limit,
        motor,
        spring,
    }: Parts,
    softness: Softness,
    h: f32,
    use_bias: bool,
) {
    let reference = match joint.kind {
        JointKind::Revolute {
            reference_angle, ..
        } => reference_angle,
        _ => 0.0,
    };
    let turning = a.i + b.i;
    if turning > 0.0 {
        let axial_mass = 1.0 / turning;
        // The angle, the spring, the motor and the limits are `a`'s against
        // `b`'s: the joint lives on the body that turns, held to what it
        // turns on.
        //
        // The spring acts in the relax pass too, as a distance joint's does:
        // it is the joint's behaviour, not an error being corrected.
        if let Some(spring) = spring.filter(|s| s.hertz > 0.0) {
            let s = joint.spring;
            let c = joint.angle_now(a, b, reference) - spring.rest;
            let cdot = a.w - b.w;
            let impulse = -s.mass_scale * axial_mass * (cdot + s.bias_rate * c)
                - s.impulse_scale * joint.impulses.spring;
            joint.impulses.spring += impulse;
            a.w += a.i * impulse;
            b.w -= b.i * impulse;
        }
        if let Some(motor) = motor {
            let cdot = a.w - b.w - motor.speed;
            let impulse = -axial_mass * cdot;
            let max = h * motor.max_torque.max(0.0);
            let old = joint.impulses.motor;
            joint.impulses.motor = (old + impulse).clamp(-max, max);
            let impulse = joint.impulses.motor - old;
            a.w += a.i * impulse;
            b.w -= b.i * impulse;
        }
        if let Some(limit) = limit {
            let angle = joint.angle_now(a, b, reference);
            // Lower: the angle kept at or above it.
            {
                let c = angle - limit.lower;
                let (bias, mass_scale, impulse_scale) = limit_terms(c, softness, h, use_bias);
                let cdot = a.w - b.w;
                let impulse =
                    -axial_mass * mass_scale * (cdot + bias) - impulse_scale * joint.impulses.lower;
                let new = (joint.impulses.lower + impulse).max(0.0);
                let impulse = new - joint.impulses.lower;
                joint.impulses.lower = new;
                a.w += a.i * impulse;
                b.w -= b.i * impulse;
            }
            // Upper: at or below it.
            {
                let c = limit.upper - angle;
                let (bias, mass_scale, impulse_scale) = limit_terms(c, softness, h, use_bias);
                let cdot = b.w - a.w;
                let impulse =
                    -axial_mass * mass_scale * (cdot + bias) - impulse_scale * joint.impulses.upper;
                let new = (joint.impulses.upper + impulse).max(0.0);
                let impulse = new - joint.impulses.upper;
                joint.impulses.upper = new;
                a.w -= a.i * impulse;
                b.w += b.i * impulse;
            }
        }
    }
    point::solve(joint, a, b, softness, use_bias);
}

/// The bias and scales of one side of a limit `c` away from it: inside the
/// limit, speculative — exactly the speed that closes the gap in a sub-step;
/// past it, pushed back softly, or not at all in the relax pass.
pub(super) fn limit_terms(c: f32, softness: Softness, h: f32, use_bias: bool) -> (f32, f32, f32) {
    if c > 0.0 {
        (c / h, 1.0, 0.0)
    } else if use_bias {
        (
            c * softness.bias_rate,
            softness.mass_scale,
            softness.impulse_scale,
        )
    } else {
        (0.0, 1.0, 0.0)
    }
}
