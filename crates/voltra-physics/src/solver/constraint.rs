//! One contact, precomputed into what the passes actually need.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::PhysicsMaterial;

use super::body::SolverBodies;
use super::cache::{ContactKey, ImpulseCache};
use super::softness::Softness;
use crate::narrow::Contact;

/// A contact in the form the solver reads it.
///
/// Everything that does not change during a step is computed once here: the
/// masses, the surface, the spring, and the separation the sub-steps will track
/// from. What the passes do change is the two accumulated impulses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactConstraint {
    /// Index of the body the normal pushes, in the step's solver bodies.
    pub a: usize,
    /// Index of the body it is pushed away from.
    pub b: usize,
    /// The pair, for the impulse cache.
    pub key: ContactKey,
    /// Unit vector pushing `a` away from `b`.
    pub normal: Vec2,
    /// Separation when the step began: negative, since a contact overlaps.
    pub base_separation: f32,
    /// `1 / (inverse_mass_a + inverse_mass_b)`.
    ///
    /// One value, not a normal mass and a tangent mass, because without
    /// rotation there is no torque arm to tell them apart. Rotation splits this
    /// in two — that is 11b-3's problem, and inventing the split now would be a
    /// second field holding the same number.
    pub effective_mass: f32,
    /// Mixed friction of the two surfaces.
    pub friction: f32,
    /// Mixed restitution of the two surfaces.
    pub restitution: f32,
    /// Approach speed along the normal when the step began, before any impulse
    /// was applied. Negative while the bodies close. Restitution needs it: the
    /// bounce is a fraction of the speed of *arrival*, not of whatever is left
    /// after the solver has removed it.
    pub relative_velocity: f32,
    /// Normal impulse accumulated so far this step.
    pub normal_impulse: f32,
    /// Friction impulse accumulated so far this step.
    pub tangent_impulse: f32,
    /// Largest normal impulse this contact has reached this step.
    ///
    /// Zero means the contact never pushed — it was reported as overlapping but
    /// separated during the sub-steps — and restitution has nothing to bounce.
    pub max_normal_impulse: f32,
    /// The spring this contact is solved with.
    pub softness: Softness,
}

impl ContactConstraint {
    /// The direction friction acts along: the normal turned a quarter turn.
    pub fn tangent(&self) -> Vec2 {
        Vec2::new(-self.normal.y, self.normal.x)
    }
}

/// Turns this step's contacts into constraints.
///
/// Contacts whose bodies both have zero inverse mass are dropped: two immovable
/// things overlapping cannot be separated by an impulse, and dividing by their
/// mass sum would be a division by zero. Dropping them here is also why nothing
/// downstream has to check.
pub fn prepare(
    contacts: &[Contact],
    bodies: &SolverBodies,
    world: &World,
    softness: (Softness, Softness),
    cache: &ImpulseCache,
    warm_starting: bool,
) -> Vec<ContactConstraint> {
    let (dynamic, immovable) = softness;

    contacts
        .iter()
        .filter_map(|contact| {
            let a = bodies.index_of(contact.a)?;
            let b = bodies.index_of(contact.b)?;
            let (body_a, body_b) = (bodies.get(a), bodies.get(b));

            let inverse_mass_sum = body_a.inverse_mass + body_b.inverse_mass;
            if inverse_mass_sum <= 0.0 {
                return None;
            }

            let (friction, restitution) = PhysicsMaterial::combine(
                world.get::<PhysicsMaterial>(contact.a),
                world.get::<PhysicsMaterial>(contact.b),
            );

            let key = (contact.a, contact.b);
            let warm = if warm_starting {
                cache.warm_start(key)
            } else {
                Default::default()
            };

            Some(ContactConstraint {
                a,
                b,
                key,
                normal: contact.normal,
                // The narrow phase reports how deep the overlap is; the solver
                // thinks in separation, which is the same number negated.
                base_separation: -contact.penetration,
                effective_mass: 1.0 / inverse_mass_sum,
                friction,
                restitution,
                relative_velocity: (body_a.velocity - body_b.velocity).dot(contact.normal),
                normal_impulse: warm.normal,
                tangent_impulse: warm.tangent,
                max_normal_impulse: 0.0,
                // Stiffer against something that cannot be pushed: all of the
                // correction has to come from the one body that can move, and a
                // soft ground is a ground bodies sink into.
                softness: if body_a.inverse_mass == 0.0 || body_b.inverse_mass == 0.0 {
                    immovable
                } else {
                    dynamic
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::cache::CachedImpulse;
    use crate::solver::params::SolverParams;
    use voltra_ecs::Entity;
    use voltra_scene::{Collider, RigidBody, Transform};

    /// A floor and a ball overlapping by 0.25, and the contact between them.
    fn scene() -> (World, Entity, Entity) {
        let mut world = World::new();

        let floor = world.spawn();
        world.insert(floor, Transform::from_translation(Vec2::new(0.0, -1.0)));
        world.insert(
            floor,
            Collider::Box {
                half_extents: Vec2::new(10.0, 1.0),
            },
        );

        let ball = world.spawn();
        world.insert(ball, Transform::from_translation(Vec2::new(0.0, 0.25)));
        world.insert(ball, Collider::Circle { radius: 0.5 });
        world.insert(ball, RigidBody::new_dynamic(1.0));

        (world, floor, ball)
    }

    fn constraints_of(world: &World, cache: &ImpulseCache, warm: bool) -> Vec<ContactConstraint> {
        let contacts = crate::step::collide(world);
        let bodies = SolverBodies::gather(world);
        let params = SolverParams::default();
        let softness = params.softness(params.sub_step(1.0 / 60.0));
        prepare(&contacts, &bodies, world, softness, cache, warm)
    }

    #[test]
    fn a_contact_becomes_one_constraint() {
        let (world, _, _) = scene();

        let constraints = constraints_of(&world, &ImpulseCache::default(), true);

        assert_eq!(constraints.len(), 1);
        assert!(
            constraints[0].base_separation < 0.0,
            "an overlap is a negative separation, got {}",
            constraints[0].base_separation
        );
        assert!((constraints[0].effective_mass - 1.0).abs() < 1e-6);
    }

    #[test]
    fn two_immovable_bodies_have_nothing_to_solve() {
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Transform::default());
        world.insert(a, Collider::Circle { radius: 1.0 });
        let b = world.spawn();
        world.insert(b, Transform::from_translation(Vec2::new(1.0, 0.0)));
        world.insert(b, Collider::Circle { radius: 1.0 });

        assert!(constraints_of(&world, &ImpulseCache::default(), true).is_empty());
    }

    #[test]
    fn a_contact_with_the_ground_is_solved_more_stiffly() {
        let (world, _, _) = scene();
        let params = SolverParams::default();
        let (dynamic, immovable) = params.softness(params.sub_step(1.0 / 60.0));

        let constraints = constraints_of(&world, &ImpulseCache::default(), true);

        assert_eq!(constraints[0].softness, immovable);
        assert_ne!(constraints[0].softness, dynamic);
    }

    #[test]
    fn the_default_surface_applies_when_no_material_is_present() {
        let (world, _, _) = scene();

        let constraints = constraints_of(&world, &ImpulseCache::default(), true);

        assert!((constraints[0].friction - 0.6).abs() < 1e-6);
        assert_eq!(constraints[0].restitution, 0.0);
    }

    #[test]
    fn a_material_on_either_body_reaches_the_constraint() {
        let (mut world, floor, ball) = scene();
        world.insert(
            floor,
            PhysicsMaterial {
                friction: 0.1,
                restitution: 0.5,
            },
        );
        world.insert(
            ball,
            PhysicsMaterial {
                friction: 0.1,
                restitution: 0.2,
            },
        );

        let constraints = constraints_of(&world, &ImpulseCache::default(), true);

        assert!((constraints[0].friction - 0.1).abs() < 1e-6);
        assert!(
            (constraints[0].restitution - 0.5).abs() < 1e-6,
            "the maximum"
        );
    }

    #[test]
    fn a_warm_constraint_starts_from_last_steps_impulse() {
        let (world, floor, ball) = scene();
        let mut cache = ImpulseCache::default();
        cache.record(
            (floor, ball),
            CachedImpulse {
                normal: 7.0,
                tangent: -2.0,
            },
        );
        cache.commit();

        let constraints = constraints_of(&world, &cache, true);

        assert_eq!(constraints[0].normal_impulse, 7.0);
        assert_eq!(constraints[0].tangent_impulse, -2.0);
    }

    #[test]
    fn warm_starting_switched_off_starts_from_zero() {
        let (world, floor, ball) = scene();
        let mut cache = ImpulseCache::default();
        cache.record(
            (floor, ball),
            CachedImpulse {
                normal: 7.0,
                tangent: -2.0,
            },
        );
        cache.commit();

        let constraints = constraints_of(&world, &cache, false);

        assert_eq!(constraints[0].normal_impulse, 0.0);
        assert_eq!(constraints[0].tangent_impulse, 0.0);
    }

    #[test]
    fn the_tangent_is_perpendicular_to_the_normal() {
        let (world, _, _) = scene();

        let constraint = constraints_of(&world, &ImpulseCache::default(), true)[0];

        assert!(constraint.tangent().dot(constraint.normal).abs() < 1e-6);
        assert!((constraint.tangent().length() - 1.0).abs() < 1e-6);
    }
}
