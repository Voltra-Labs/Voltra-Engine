//! The three passes that turn constraints into velocities.
//!
//! Every impulse here has two halves. `v ± P·inv_mass` moves the body, and
//! `w ± inv_inertia · cross(r, P)` turns it — the same impulse, seen from the
//! centre of mass and from the arm to the contact.

use voltra_render::glam::Vec2;

use super::body::{SolverBodies, SolverBody};
use super::constraint::{perpendicular, ContactConstraint, ContactPoint};

/// Applies the impulses carried over from the previous step.
///
/// This is warm starting doing its work: the velocities begin the step already
/// holding most of the force the contacts will need, so the sub-steps correct a
/// small error instead of rediscovering the whole one.
pub fn warm_start(constraints: &[ContactConstraint], bodies: &mut SolverBodies) {
    for constraint in constraints {
        let tangent = constraint.tangent();
        let (a, b) = bodies.pair_mut(constraint.a, constraint.b);

        for point in constraint.points() {
            let impulse =
                constraint.normal * point.normal_impulse + tangent * point.tangent_impulse;
            apply(a, b, point, impulse);
        }
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
        let normal = constraint.normal;
        let tangent = constraint.tangent();
        let softness = constraint.softness;
        let friction = constraint.friction;
        let (a, b) = bodies.pair_mut(constraint.a, constraint.b);

        // The anchors turn with their bodies, so the arms the separation is
        // measured along are the current ones rather than the ones the step
        // started with. The *Jacobian* deliberately keeps the fixed anchors —
        // see the friction solve below.
        let rotation_a = Vec2::from_angle(a.delta_rotation);
        let rotation_b = Vec2::from_angle(b.delta_rotation);
        let delta_position = a.delta_position - b.delta_position;

        for point in constraint.points_mut() {
            // Box2D v3: s = dot(normal, dp + (rsB − rsA)) + base_separation,
            // with the normal held constant for the whole step.
            let moved_a = rotation_a.rotate(point.anchor_a);
            let moved_b = rotation_b.rotate(point.anchor_b);
            let separation =
                normal.dot(delta_position + (moved_a - moved_b)) + point.base_separation;

            let (bias, mass_scale, impulse_scale) = if separation > 0.0 {
                // Speculative: they were overlapping when the step began but
                // have separated within it. Allow exactly the velocity that
                // closes the gap and no more, so the solve does not push
                // through it.
                (separation / h, 1.0, 0.0)
            } else if use_bias {
                (
                    (softness.mass_scale * softness.bias_rate * separation).max(-max_push_speed),
                    softness.mass_scale,
                    softness.impulse_scale,
                )
            } else {
                (0.0, 1.0, 0.0)
            };

            let normal_velocity = relative_velocity(a, b, point).dot(normal);
            let impulse = -point.normal_mass * (mass_scale * normal_velocity + bias)
                - impulse_scale * point.normal_impulse;

            // Clamp the *accumulated* impulse, never the increment: that is
            // what lets a later pass undo an earlier overshoot, and clamping
            // the increment instead is a classic source of jitter.
            let total = (point.normal_impulse + impulse).max(0.0);
            let applied = total - point.normal_impulse;
            point.normal_impulse = total;
            point.max_normal_impulse = point.max_normal_impulse.max(total);

            apply(a, b, point, normal * applied);
        }

        if use_bias {
            continue;
        }

        for point in constraint.points_mut() {
            // Friction uses the anchors as they were, not as they have turned.
            // Box2D splits them the same way: an anchor that drifts with the
            // body turns static friction into a slow slide.
            let tangent_velocity = relative_velocity(a, b, point).dot(tangent);
            let impulse = -point.tangent_mass * tangent_velocity;

            // Coulomb: friction can resist at most `μ` times the force pressing
            // the surfaces together, and it acts either way along the tangent.
            // The limit is this point's own normal impulse — one end of a face
            // can be pressed hard while the other is barely touching.
            let limit = friction * point.normal_impulse;
            let total = (point.tangent_impulse + impulse).clamp(-limit, limit);
            let applied = total - point.tangent_impulse;
            point.tangent_impulse = total;

            apply(a, b, point, tangent * applied);
        }
    }
}

/// Gives back the fraction of the approach speed the surfaces are supposed to
/// return, in a pass of its own.
///
/// It runs after relaxation because relaxation exists to remove velocity the
/// bias added, and a bounce applied before it would be removed with the rest.
///
/// Two points are skipped: one that arrived slower than `threshold`, which is
/// what stops a resting body from bouncing forever on its own numerical noise,
/// and one that never pushed at all, which is a point that separated during the
/// sub-steps and has nothing to bounce off.
pub fn apply_restitution(
    constraints: &mut [ContactConstraint],
    bodies: &mut SolverBodies,
    threshold: f32,
) {
    for constraint in constraints.iter_mut() {
        if constraint.restitution <= 0.0 {
            continue;
        }

        let normal = constraint.normal;
        let restitution = constraint.restitution;
        let (a, b) = bodies.pair_mut(constraint.a, constraint.b);

        for point in constraint.points_mut() {
            if point.relative_velocity > -threshold || point.max_normal_impulse <= 0.0 {
                continue;
            }

            let normal_velocity = relative_velocity(a, b, point).dot(normal);
            let impulse =
                -point.normal_mass * (normal_velocity + restitution * point.relative_velocity);

            let total = (point.normal_impulse + impulse).max(0.0);
            let applied = total - point.normal_impulse;
            point.normal_impulse = total;

            apply(a, b, point, normal * applied);
        }
    }
}

/// The velocity of `b`'s surface relative to `a`'s, at this point.
///
/// `v + w × r` at each body: a spinning body presents a different speed at each
/// end of the same face, which is the whole reason a manifold has two points.
fn relative_velocity(a: &SolverBody, b: &SolverBody, point: &ContactPoint) -> Vec2 {
    (a.velocity + perpendicular(a.angular_velocity, point.anchor_a))
        - (b.velocity + perpendicular(b.angular_velocity, point.anchor_b))
}

/// Applies `impulse` to `a` and its negation to `b`, linearly and angularly.
fn apply(a: &mut SolverBody, b: &mut SolverBody, point: &ContactPoint, impulse: Vec2) {
    a.velocity += impulse * a.inverse_mass;
    a.angular_velocity += a.inverse_inertia * point.anchor_a.perp_dot(impulse);
    b.velocity -= impulse * b.inverse_mass;
    b.angular_velocity -= b.inverse_inertia * point.anchor_b.perp_dot(impulse);
}
