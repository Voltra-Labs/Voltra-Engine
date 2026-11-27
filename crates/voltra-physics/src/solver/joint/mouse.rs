//! A mouse joint: `a`'s anchor pulled towards a target by a spring no
//! stronger than its `max_force` (Box2D's `b2SolveMouseJoint`).
//!
//! Box2D's pulls one body to a world point; the target here is `b`'s
//! anchor, the world point when `b` is the world — the general case is the
//! same constraint, so a body can as well be pulled to another one.
//! Box2D's own mouse joint also damps the body's spin with a fixed spring;
//! that is left out, so a body picked off its centre swings about the grip
//! as it would hanging from one.

use voltra_scene::JointKind;

use super::{point, JointConstraint, Side};
use crate::solver::Softness;

/// One solve. The spring pulls in the relax pass too, as Box2D's does: the
/// pull is the joint's behaviour, not an error being corrected. With no
/// frequency it pulls as hard as a joint holds its anchors.
pub(super) fn solve(
    joint: &mut JointConstraint,
    a: &mut Side,
    b: &mut Side,
    softness: Softness,
    h: f32,
    use_bias: bool,
) {
    let JointKind::Mouse {
        max_force, hertz, ..
    } = joint.kind
    else {
        return;
    };
    let soft = if hertz > 0.0 {
        Some(joint.spring)
    } else {
        use_bias.then_some(softness)
    };
    // `max` of a NaN is the other side: a broken force pulls with none.
    let max_impulse = max_force.max(0.0) * h;
    point::hold(joint, a, b, soft, max_impulse);
}
