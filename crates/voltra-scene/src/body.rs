//! What moves, and how much it resists being moved.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;

/// How a body responds to time and, later, to contacts.
///
/// The three every engine converged on. Kinematic is not a special case of the
/// other two: it moves under its own velocity like a dynamic body, and is
/// immovable by contacts like a static one, which is what a moving platform is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BodyType {
    /// Never moves. Walls, floors.
    ///
    /// The default because a `RigidBody` added from the inspector must not send
    /// the sprite falling off the screen before a mass has been typed.
    #[default]
    Static,
    /// Moves by its velocity, ignoring gravity and — in 11b-2 — impulses.
    Kinematic,
    /// Moves by velocity and gravity, and will be pushed by contacts.
    Dynamic,
}

/// A body in the simulation.
///
/// Stores `inverse_mass` rather than mass, as Box2D does: every formula that
/// uses it divides by mass, and infinite mass — anything that cannot be pushed
/// — is `0.0` instead of a branch repeated at each of them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RigidBody {
    pub body_type: BodyType,
    pub velocity: Vec2,
    /// `1.0 / mass`, or `0.0` for a body that cannot be pushed.
    pub inverse_mass: f32,
    /// Multiplier on the world's gravity. `0.0` for a floating body.
    pub gravity_scale: f32,
    /// Fraction of speed shed per second. `0.0` keeps all of it.
    pub linear_damping: f32,
}

impl Default for RigidBody {
    fn default() -> Self {
        Self {
            body_type: BodyType::Static,
            velocity: Vec2::ZERO,
            inverse_mass: 0.0,
            gravity_scale: 1.0,
            linear_damping: 0.0,
        }
    }
}

impl RigidBody {
    /// A dynamic body of `mass`.
    ///
    /// A mass of zero or less means "cannot be pushed" rather than "infinitely
    /// light": `1.0 / 0.0` is infinity, and an infinite inverse mass sends the
    /// body out of the world on the first contact it takes. Every engine reads
    /// zero the same way.
    pub fn new_dynamic(mass: f32) -> Self {
        Self {
            body_type: BodyType::Dynamic,
            inverse_mass: if mass > 0.0 { 1.0 / mass } else { 0.0 },
            ..Default::default()
        }
    }

    /// A body that never moves.
    pub fn new_static() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_body_is_static_and_immovable() {
        // Static, not Dynamic: a component added by a click in the inspector
        // must not make the sprite fall off the screen before anyone has typed
        // a mass. Unity, Godot and Box2D all default to the inert body too.
        let body = RigidBody::default();

        assert_eq!(body.body_type, BodyType::Static);
        assert_eq!(body.inverse_mass, 0.0);
        assert_eq!(body.velocity, Vec2::ZERO);
    }

    #[test]
    fn a_dynamic_body_stores_the_reciprocal_of_its_mass() {
        let body = RigidBody::new_dynamic(4.0);

        assert_eq!(body.body_type, BodyType::Dynamic);
        assert_eq!(body.inverse_mass, 0.25);
    }

    #[test]
    fn a_zero_mass_body_is_infinitely_massive_not_infinitely_light() {
        // 1/0 is inf, and an inf inverse mass makes a body react to the
        // smallest impulse by leaving the world. Zero mass means "cannot be
        // moved", which is what every engine means by it.
        let body = RigidBody::new_dynamic(0.0);

        assert_eq!(body.inverse_mass, 0.0);
        assert!(body.inverse_mass.is_finite());
    }

    #[test]
    fn a_negative_mass_is_treated_as_zero() {
        // A scene file is external input and can say anything.
        assert_eq!(RigidBody::new_dynamic(-5.0).inverse_mass, 0.0);
    }

    #[test]
    fn a_static_body_has_no_inverse_mass() {
        assert_eq!(RigidBody::new_static().inverse_mass, 0.0);
        assert_eq!(RigidBody::new_static().body_type, BodyType::Static);
    }

    #[test]
    fn a_body_round_trips_through_ron() {
        let body = RigidBody {
            body_type: BodyType::Kinematic,
            velocity: Vec2::new(1.5, -2.5),
            inverse_mass: 0.5,
            gravity_scale: 2.0,
            linear_damping: 0.1,
        };

        let text = ron::to_string(&body).expect("serialise");
        let back: RigidBody = ron::from_str(&text).expect("deserialise");

        assert_eq!(back, body);
    }
}
