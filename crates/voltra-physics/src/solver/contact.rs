//! The three passes that turn constraints into velocities.

use super::body::SolverBodies;
use super::constraint::ContactConstraint;

/// Applies the impulses carried over from the previous step.
///
/// This is warm starting doing its work: the velocities begin the step already
/// holding most of the force the contacts will need, so the sub-steps correct a
/// small error instead of rediscovering the whole one.
pub fn warm_start(constraints: &[ContactConstraint], bodies: &mut SolverBodies) {
    for constraint in constraints {
        let impulse = constraint.normal * constraint.normal_impulse
            + constraint.tangent() * constraint.tangent_impulse;

        let (a, b) = bodies.pair_mut(constraint.a, constraint.b);
        a.velocity += impulse * a.inverse_mass;
        b.velocity -= impulse * b.inverse_mass;
    }
}

/// One pass of the contact solve.
///
/// Run with `use_bias` true inside the sub-steps, where the soft constraint
/// pushes overlapping bodies apart, and once more with it false afterwards —
/// the relax pass, which takes the energy that push added back out of the
/// velocities and the accumulated impulses without moving anything.
///
/// **Friction is solved in the relax pass only.** That is what the Box2D source
/// does, and the reason is that a friction impulse computed while the normal
/// solve is injecting separation velocity is scaled by that separation rather
/// than by contact force.
pub fn solve(
    constraints: &mut [ContactConstraint],
    bodies: &mut SolverBodies,
    h: f32,
    use_bias: bool,
    max_push_speed: f32,
) {
    for constraint in constraints.iter_mut() {
        let (a, b) = bodies.pair_mut(constraint.a, constraint.b);

        // Where the two are now. The normal is held constant for the whole
        // step, so the separation follows from how far each body has moved
        // since it was measured — no second narrow phase per sub-step.
        let separation = constraint.base_separation
            + (a.delta_position - b.delta_position).dot(constraint.normal);

        let (bias, mass_scale, impulse_scale) = if separation > 0.0 {
            // Speculative: they were overlapping when the step began but have
            // separated within it. Allow exactly the velocity that closes the
            // gap and no more, so the solve does not push through the gap.
            (separation / h, 1.0, 0.0)
        } else if use_bias {
            (
                (constraint.softness.mass_scale * constraint.softness.bias_rate * separation)
                    .max(-max_push_speed),
                constraint.softness.mass_scale,
                constraint.softness.impulse_scale,
            )
        } else {
            (0.0, 1.0, 0.0)
        };

        let normal_velocity = (a.velocity - b.velocity).dot(constraint.normal);
        let impulse = -constraint.effective_mass * (mass_scale * normal_velocity + bias)
            - impulse_scale * constraint.normal_impulse;

        // Clamp the *accumulated* impulse, never the increment: that is what
        // lets a later pass undo an earlier overshoot, and clamping the
        // increment instead is a classic source of jitter.
        let total = (constraint.normal_impulse + impulse).max(0.0);
        let applied = total - constraint.normal_impulse;
        constraint.normal_impulse = total;
        constraint.max_normal_impulse = constraint.max_normal_impulse.max(total);

        let push = constraint.normal * applied;
        a.velocity += push * a.inverse_mass;
        b.velocity -= push * b.inverse_mass;

        if use_bias {
            continue;
        }

        let tangent = constraint.tangent();
        let tangent_velocity = (a.velocity - b.velocity).dot(tangent);
        let impulse = -constraint.effective_mass * tangent_velocity;

        // Coulomb: friction can resist at most `μ` times the force pressing the
        // surfaces together, and it acts in either direction along the tangent.
        let limit = constraint.friction * constraint.normal_impulse;
        let total = (constraint.tangent_impulse + impulse).clamp(-limit, limit);
        let applied = total - constraint.tangent_impulse;
        constraint.tangent_impulse = total;

        let rub = tangent * applied;
        a.velocity += rub * a.inverse_mass;
        b.velocity -= rub * b.inverse_mass;
    }
}

/// Gives back the fraction of the approach speed the surfaces are meant to
/// return.
///
/// One pass, after relaxation, because a bounce applied inside the sub-steps
/// would be flattened by the solves that follow it.
///
/// Two contacts are skipped: one that arrived slower than `threshold`, which is
/// what stops a resting body from bouncing forever on its own numerical noise,
/// and one that never pushed at all, which is a contact that separated during
/// the sub-steps and has nothing to bounce off.
pub fn apply_restitution(
    constraints: &mut [ContactConstraint],
    bodies: &mut SolverBodies,
    threshold: f32,
) {
    for constraint in constraints.iter_mut() {
        if constraint.restitution <= 0.0
            || constraint.relative_velocity > -threshold
            || constraint.max_normal_impulse <= 0.0
        {
            continue;
        }

        let (a, b) = bodies.pair_mut(constraint.a, constraint.b);

        let normal_velocity = (a.velocity - b.velocity).dot(constraint.normal);
        let impulse = -constraint.effective_mass
            * (normal_velocity + constraint.restitution * constraint.relative_velocity);

        let total = (constraint.normal_impulse + impulse).max(0.0);
        let applied = total - constraint.normal_impulse;
        constraint.normal_impulse = total;

        let push = constraint.normal * applied;
        a.velocity += push * a.inverse_mass;
        b.velocity -= push * b.inverse_mass;
    }
}
