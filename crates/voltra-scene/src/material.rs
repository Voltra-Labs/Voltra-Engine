//! How a surface rubs, and how it bounces.

use serde::{Deserialize, Serialize};

/// The surface properties of a collider.
///
/// A component of its own rather than fields on [`Collider`] or [`RigidBody`].
/// `Collider` is an enum, so two fields there are duplicated in every variant
/// and in every shape added later; `RigidBody` would leave static geometry — a
/// collider with no body, which the solver already has to handle — with no
/// surface at all. Unity's `PhysicsMaterial2D` is the same separation, adapted
/// to composition rather than to an asset reference.
///
/// [`Collider`]: crate::Collider
/// [`RigidBody`]: crate::RigidBody
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsMaterial {
    /// Resistance to sliding. Box2D's default of `0.6` is a wooden crate.
    pub friction: f32,
    /// Fraction of approach speed returned on impact. `0.0` does not bounce.
    pub restitution: f32,
}

impl Default for PhysicsMaterial {
    fn default() -> Self {
        Self {
            friction: 0.6,
            restitution: 0.0,
        }
    }
}

impl PhysicsMaterial {
    /// The surface of a contact between two colliders, mixed and clamped.
    ///
    /// `sqrt(fa · fb)` and `max(ra, rb)`, which is what Box2D mixes: the
    /// geometric mean lets one slippery surface make the pair slippery, and the
    /// maximum lets one bouncy surface make the pair bounce. Averaging both
    /// would mean ice on rubber behaves like neither.
    ///
    /// A missing component is the default surface, so an entity nobody has
    /// given a material still rubs.
    ///
    /// Both values are clamped here rather than trusted, because a scene file
    /// is external input: a negative friction reverses the friction impulse,
    /// and a restitution above one adds energy to every bounce until the body
    /// leaves the world.
    pub fn combine(a: Option<&Self>, b: Option<&Self>) -> (f32, f32) {
        let default = Self::default();
        let a = a.unwrap_or(&default);
        let b = b.unwrap_or(&default);

        let friction = (a.friction.max(0.0) * b.friction.max(0.0)).sqrt();
        let restitution = a.restitution.max(b.restitution).clamp(0.0, 1.0);
        (friction, restitution)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_material_is_the_default_surface() {
        let (friction, restitution) = PhysicsMaterial::combine(None, None);

        assert!((friction - 0.6).abs() < 1e-6, "{friction}");
        assert_eq!(restitution, 0.0);
    }

    #[test]
    fn friction_is_the_geometric_mean_and_restitution_the_maximum() {
        let ice = PhysicsMaterial {
            friction: 0.04,
            restitution: 0.0,
        };
        let rubber = PhysicsMaterial {
            friction: 0.9,
            restitution: 0.8,
        };

        let (friction, restitution) = PhysicsMaterial::combine(Some(&ice), Some(&rubber));

        assert!(
            (friction - (0.04f32 * 0.9).sqrt()).abs() < 1e-6,
            "{friction}"
        );
        assert!((restitution - 0.8).abs() < 1e-6, "{restitution}");
    }

    #[test]
    fn a_scene_cannot_ask_for_negative_friction_or_a_restitution_above_one() {
        let bad = PhysicsMaterial {
            friction: -5.0,
            restitution: 9.0,
        };

        let (friction, restitution) = PhysicsMaterial::combine(Some(&bad), Some(&bad));

        assert_eq!(friction, 0.0);
        assert_eq!(restitution, 1.0);
    }

    #[test]
    fn one_slippery_surface_makes_the_pair_slippery() {
        let ice = PhysicsMaterial {
            friction: 0.0,
            restitution: 0.0,
        };
        let rough = PhysicsMaterial {
            friction: 1.0,
            restitution: 0.0,
        };

        let (friction, _) = PhysicsMaterial::combine(Some(&ice), Some(&rough));

        assert_eq!(friction, 0.0);
    }

    #[test]
    fn a_material_round_trips_through_ron() {
        let material = PhysicsMaterial {
            friction: 0.25,
            restitution: 0.5,
        };

        let text = ron::to_string(&material).expect("serialise");
        let back: PhysicsMaterial = ron::from_str(&text).expect("deserialise");

        assert_eq!(back, material);
    }
}
