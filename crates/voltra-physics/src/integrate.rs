//! Moving bodies forward by one sub-step.
//!
//! Split in two because the solver has to run between the halves: velocities
//! are integrated, the contacts are solved against those velocities, and only
//! then do positions move. Doing both at once — which is what 11b-1 did, with
//! no solver to fit in between — would apply gravity to a position the contact
//! was about to reject.

use voltra_render::glam::Vec2;
use voltra_scene::BodyType;

use crate::solver::SolverBodies;

/// Applies gravity and damping to every dynamic body.
///
/// Semi-implicit (symplectic) Euler: velocity is updated from the acceleration
/// *first*, then [`integrate_positions`] moves from the new velocity. Explicit
/// Euler — position from the old velocity — injects energy, and a stack of
/// boxes slowly climbs. The correct order costs nothing but being written down.
pub fn integrate_velocities(bodies: &mut SolverBodies, gravity: Vec2, h: f32) {
    for body in bodies.iter_mut() {
        // No gravity, no damping for a kinematic body: it moves exactly as it
        // was told to, which is what makes it usable as a platform. A static
        // body does not move at all.
        if body.body_type != BodyType::Dynamic {
            continue;
        }

        let accelerated = body.velocity + gravity * body.gravity_scale * h;
        // Clamped at zero: `1 − damping·h` goes negative once the damping is
        // large enough, which turns a brake into a catapult. A scene file can
        // contain any number.
        let retained = (1.0 - body.linear_damping * h).max(0.0);
        body.velocity = accelerated * retained;
    }
}

/// Accumulates each body's move for this sub-step.
///
/// The move lands in `delta_position` rather than in the `Transform`, so the
/// solver can track a contact's separation through the sub-steps arithmetically
/// and the world is written exactly once per step.
pub fn integrate_positions(bodies: &mut SolverBodies, h: f32) {
    for body in bodies.iter_mut() {
        if body.body_type == BodyType::Static {
            continue;
        }
        body.delta_position += body.velocity * h;
    }
}
