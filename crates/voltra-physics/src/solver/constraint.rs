//! One contact, precomputed into what the passes actually need.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::{PhysicsMaterial, Transform};

use super::body::SolverBodies;
use super::cache::{ContactKey, ImpulseCache};
use super::softness::Softness;
use crate::narrow::Contact;

/// One point of a contact, with everything that varies across the manifold.
///
/// A manifold's two points share a normal, a surface and a spring, and share
/// nothing else: each has its own arm to the two centres of mass, and therefore
/// its own effective mass, its own accumulated impulses and its own warm-start
/// identity.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ContactPoint {
    /// The point relative to `a`'s centre, in world orientation at the moment
    /// the step began. The torque arm, and half of the separation tracking.
    pub anchor_a: Vec2,
    /// The same, relative to `b`'s centre.
    pub anchor_b: Vec2,
    /// Separation with the anchors' own offset removed, so a sub-step can
    /// recover the current separation from how far the bodies have moved and
    /// turned. Box2D's `adjustedSeparation`.
    pub base_separation: f32,
    /// `1 / (mA + mB + iA·rnA² + iB·rnB²)`.
    ///
    /// 11b-2 had one `effective_mass` for both axes, which was right only
    /// because no contact could apply torque. An arm makes the normal and the
    /// tangent resist differently, so they are two numbers now.
    pub normal_mass: f32,
    /// The same along the tangent, with `rt = cross(r, tangent)`.
    pub tangent_mass: f32,
    /// Approach speed along the normal *at this point* when the step began,
    /// before any impulse. Negative while the bodies close. Restitution needs
    /// it: a bounce is a fraction of the speed of arrival, not of whatever is
    /// left after the solver has removed it.
    pub relative_velocity: f32,
    /// Normal impulse accumulated so far this step.
    pub normal_impulse: f32,
    /// Friction impulse accumulated so far this step.
    pub tangent_impulse: f32,
    /// Largest normal impulse this point has reached this step.
    ///
    /// Zero means it never pushed — reported as overlapping, then separated
    /// during the sub-steps — and restitution has nothing to bounce.
    pub max_normal_impulse: f32,
    /// Which features made this point, from the narrow phase.
    pub id: u16,
}

/// A contact in the form the solver reads it.
///
/// Everything that does not change during a step is computed once here: the
/// masses, the surface, the spring, and the separation the sub-steps track
/// from. What the passes change is the impulses, point by point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactConstraint {
    /// Index of the body the normal pushes, in the step's solver bodies.
    pub a: usize,
    /// Index of the body it is pushed away from.
    pub b: usize,
    /// The pair, which the cache keys on together with each point's id.
    pub key: (voltra_ecs::Entity, voltra_ecs::Entity),
    /// Unit vector pushing `a` away from `b`, shared by every point.
    pub normal: Vec2,
    /// Mixed friction of the two surfaces.
    pub friction: f32,
    /// Mixed restitution of the two surfaces.
    pub restitution: f32,
    /// The spring this contact is solved with.
    pub softness: Softness,
    points: [ContactPoint; 2],
    count: u8,
}

impl ContactConstraint {
    /// The direction friction acts along: the normal turned a quarter turn.
    pub fn tangent(&self) -> Vec2 {
        Vec2::new(-self.normal.y, self.normal.x)
    }

    pub fn points(&self) -> &[ContactPoint] {
        &self.points[..self.count as usize]
    }

    pub fn points_mut(&mut self) -> &mut [ContactPoint] {
        &mut self.points[..self.count as usize]
    }

    /// The cache key of one of its points.
    pub fn key_of(&self, point: &ContactPoint) -> ContactKey {
        (self.key.0, self.key.1, point.id)
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

            if body_a.inverse_mass + body_b.inverse_mass <= 0.0 {
                return None;
            }

            let (friction, restitution) = PhysicsMaterial::combine(
                world.get::<PhysicsMaterial>(contact.a),
                world.get::<PhysicsMaterial>(contact.b),
            );

            // The centres the arms are measured from. A body with no transform
            // cannot be part of a contact — `collide` needs one to have found
            // the overlap at all — so this is the position it was collided at.
            let centre_a = centre(world, contact.a);
            let centre_b = centre(world, contact.b);

            let normal = contact.normal();
            let tangent = Vec2::new(-normal.y, normal.x);

            let mut points = [ContactPoint::default(); 2];
            let mut count = 0;
            for (slot, point) in points.iter_mut().zip(contact.points()) {
                let anchor_a = point.point - centre_a;
                let anchor_b = point.point - centre_b;

                let warm = if warm_starting {
                    cache.warm_start((contact.a, contact.b, point.id))
                } else {
                    Default::default()
                };

                // Velocity at the point, not at the centre: a spinning body
                // presents a different speed at each end of its face.
                let velocity_a = body_a.velocity + perpendicular(body_a.angular_velocity, anchor_a);
                let velocity_b = body_b.velocity + perpendicular(body_b.angular_velocity, anchor_b);
                let relative = velocity_a - velocity_b;

                *slot = ContactPoint {
                    anchor_a,
                    anchor_b,
                    // The anchors' own offset along the normal is removed here
                    // so the sub-steps can add back a *moved* one. `a − b`,
                    // matching how the solve differences the delta positions —
                    // Box2D writes it `b − a` because its normal points the
                    // other way, and mixing the two orders leaves every contact
                    // reporting twice the arm as overlap.
                    base_separation: point.separation - (anchor_a - anchor_b).dot(normal),
                    normal_mass: effective_mass(body_a, body_b, anchor_a, anchor_b, normal),
                    tangent_mass: effective_mass(body_a, body_b, anchor_a, anchor_b, tangent),
                    relative_velocity: relative.dot(normal),
                    normal_impulse: warm.normal,
                    tangent_impulse: warm.tangent,
                    max_normal_impulse: 0.0,
                    id: point.id,
                };
                count += 1;
            }

            Some(ContactConstraint {
                a,
                b,
                key: (contact.a, contact.b),
                normal,
                friction,
                restitution,
                // Stiffer against something that cannot be pushed: all of the
                // correction has to come from the one body that can move, and a
                // soft ground is a ground bodies sink into.
                softness: if body_a.inverse_mass == 0.0 || body_b.inverse_mass == 0.0 {
                    immovable
                } else {
                    dynamic
                },
                points,
                count,
            })
        })
        .collect()
}

/// `1 / (mA + mB + iA·rnA² + iB·rnB²)` along `axis`.
///
/// The rotational terms are what an arm costs: a push far from the centre
/// spends part of itself spinning the body, so the contact behaves as if it
/// were heavier. A zero sum gives zero rather than an infinity, as Box2D does —
/// it can only happen for two bodies that cannot move, which `prepare` has
/// already dropped.
fn effective_mass(
    a: &super::body::SolverBody,
    b: &super::body::SolverBody,
    anchor_a: Vec2,
    anchor_b: Vec2,
    axis: Vec2,
) -> f32 {
    let rn_a = anchor_a.perp_dot(axis);
    let rn_b = anchor_b.perp_dot(axis);
    let sum = a.inverse_mass
        + b.inverse_mass
        + a.inverse_inertia * rn_a * rn_a
        + b.inverse_inertia * rn_b * rn_b;

    if sum > 0.0 {
        1.0 / sum
    } else {
        0.0
    }
}

/// `w × r`, the velocity a spin gives a point offset by `r`.
pub(super) fn perpendicular(angular_velocity: f32, arm: Vec2) -> Vec2 {
    Vec2::new(-angular_velocity * arm.y, angular_velocity * arm.x)
}

/// Where a body's centre of mass is. The transform's translation: nothing in
/// this engine offsets a collider from its entity.
fn centre(world: &World, entity: voltra_ecs::Entity) -> Vec2 {
    world
        .get::<Transform>(entity)
        .map(|transform| transform.translation)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::cache::CachedImpulse;
    use crate::solver::params::SolverParams;
    use voltra_ecs::Entity;
    use voltra_scene::{Collider, RigidBody};

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

    /// A floor and a box resting flat on it, which is a two-point manifold.
    fn stacked() -> (World, Entity, Entity) {
        let mut world = World::new();

        let floor = world.spawn();
        world.insert(floor, Transform::from_translation(Vec2::new(0.0, -1.0)));
        world.insert(
            floor,
            Collider::Box {
                half_extents: Vec2::new(10.0, 1.0),
            },
        );

        let crate_ = world.spawn();
        world.insert(crate_, Transform::from_translation(Vec2::new(0.0, 0.4)));
        world.insert(
            crate_,
            Collider::Box {
                half_extents: Vec2::splat(0.5),
            },
        );
        world.insert(crate_, RigidBody::new_dynamic(1.0));

        (world, floor, crate_)
    }

    fn constraints_of(world: &World, cache: &ImpulseCache, warm: bool) -> Vec<ContactConstraint> {
        let contacts = crate::step::collide(world).contacts;
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
        assert_eq!(constraints[0].points().len(), 1);
        // `base_separation` is not the separation: the anchors' offset has been
        // taken out of it, and the solve puts a moved one back.
        // `the_base_separation_reproduces_the_real_one_before_anything_moves`
        // is what pins the round trip.
        assert!(constraints[0].points()[0].normal_mass > 0.0);
    }

    #[test]
    fn a_two_point_manifold_becomes_a_two_point_constraint() {
        let (world, _, _) = stacked();

        let constraints = constraints_of(&world, &ImpulseCache::default(), true);

        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0].points().len(), 2);
        assert_ne!(constraints[0].points()[0].id, constraints[0].points()[1].id);
    }

    #[test]
    fn an_anchor_is_the_contact_point_relative_to_each_centre() {
        let (world, floor, ball) = stacked();
        let centre_a = world.get::<Transform>(floor).expect("floor").translation;
        let centre_b = world.get::<Transform>(ball).expect("crate").translation;

        let constraints = constraints_of(&world, &ImpulseCache::default(), true);

        for point in constraints[0].points() {
            let from_a = centre_a + point.anchor_a;
            let from_b = centre_b + point.anchor_b;
            assert!((from_a - from_b).length() < 1e-5, "{point:?}");
        }
    }

    #[test]
    fn a_contact_through_the_centre_keeps_the_old_effective_mass() {
        // With r × n = 0 the arm contributes nothing and the normal mass is
        // 1/(mA + mB) exactly, which is what 11b-2 computed. The rotation terms
        // must not change the head-on case.
        let (world, _, _) = scene();

        let constraints = constraints_of(&world, &ImpulseCache::default(), true);

        let point = constraints[0].points()[0];
        assert!(
            (point.normal_mass - 1.0).abs() < 1e-5,
            "{}",
            point.normal_mass
        );
    }

    #[test]
    fn an_off_centre_contact_is_heavier_than_a_central_one() {
        // k = mA + mB + iA·rnA² + iB·rnB², so an arm can only make the point
        // harder to push.
        let (world, _, _) = stacked();

        let constraints = constraints_of(&world, &ImpulseCache::default(), true);

        for point in constraints[0].points() {
            assert!(point.normal_mass < 1.0, "{point:?}");
            assert!(point.normal_mass > 0.0, "{point:?}");
        }
    }

    #[test]
    fn the_tangent_mass_uses_the_tangent_arm() {
        // A ball resting on the floor touches directly below its centre: no arm
        // about the normal, a full radius of arm about the tangent. So a push
        // straight down meets the whole mass and a rub sideways also has to
        // spin the ball, which costs more.
        let (world, _, _) = scene();

        let constraints = constraints_of(&world, &ImpulseCache::default(), true);

        let point = constraints[0].points()[0];
        assert!(
            point.tangent_mass < point.normal_mass - 1e-3,
            "{point:?} — friction must feel the spin"
        );
    }

    #[test]
    fn the_base_separation_reproduces_the_real_one_before_anything_moves() {
        // The anchors' offset is taken out here and added back by the solve.
        // Getting the order wrong reports twice the arm as overlap, which no
        // unit test of the formula alone would catch.
        let (world, _, _) = stacked();

        let constraints = constraints_of(&world, &ImpulseCache::default(), true);

        let constraint = constraints[0];
        for point in constraint.points() {
            let recovered =
                constraint.normal.dot(point.anchor_a - point.anchor_b) + point.base_separation;
            assert!((recovered + 0.1).abs() < 1e-5, "{recovered} for {point:?}");
        }
    }

    #[test]
    fn a_spinning_body_arrives_faster_at_the_far_end_of_its_face() {
        // The approach speed is measured at the point, so a spin makes the two
        // ends of one manifold close at different rates.
        let (mut world, _, crate_) = stacked();
        world
            .get_mut::<RigidBody>(crate_)
            .expect("the crate")
            .angular_velocity = 3.0;

        let constraints = constraints_of(&world, &ImpulseCache::default(), true);

        let speeds: Vec<f32> = constraints[0]
            .points()
            .iter()
            .map(|point| point.relative_velocity)
            .collect();
        assert!((speeds[0] - speeds[1]).abs() > 1e-3, "{speeds:?}");
    }

    #[test]
    fn each_point_is_warm_started_from_its_own_id() {
        let (world, floor, crate_) = stacked();
        let ids: Vec<u16> = constraints_of(&world, &ImpulseCache::default(), true)[0]
            .points()
            .iter()
            .map(|point| point.id)
            .collect();

        let mut cache = ImpulseCache::default();
        cache.record(
            (floor, crate_, ids[0]),
            CachedImpulse {
                normal: 7.0,
                tangent: -2.0,
            },
        );
        cache.commit();

        let constraints = constraints_of(&world, &cache, true);

        assert_eq!(constraints[0].points()[0].normal_impulse, 7.0);
        assert_eq!(constraints[0].points()[0].tangent_impulse, -2.0);
        assert_eq!(
            constraints[0].points()[1].normal_impulse,
            0.0,
            "the other point kept its own history, which is none"
        );
    }

    #[test]
    fn a_point_whose_feature_changed_starts_cold() {
        // An impulse belongs to a contact between two features. When the box
        // tips onto another corner that contact is gone, and inheriting its
        // impulse would push the new one with a force nothing measured.
        let (world, floor, crate_) = stacked();
        let mut cache = ImpulseCache::default();
        cache.record(
            (floor, crate_, 0xBEEF),
            CachedImpulse {
                normal: 7.0,
                tangent: 0.0,
            },
        );
        cache.commit();

        let constraints = constraints_of(&world, &cache, true);

        for point in constraints[0].points() {
            assert_eq!(point.normal_impulse, 0.0, "{point:?}");
        }
    }

    #[test]
    fn warm_starting_switched_off_starts_from_zero() {
        let (world, floor, crate_) = stacked();
        let ids: Vec<u16> = constraints_of(&world, &ImpulseCache::default(), true)[0]
            .points()
            .iter()
            .map(|point| point.id)
            .collect();

        let mut cache = ImpulseCache::default();
        cache.record(
            (floor, crate_, ids[0]),
            CachedImpulse {
                normal: 7.0,
                tangent: -2.0,
            },
        );
        cache.commit();

        let constraints = constraints_of(&world, &cache, false);

        assert_eq!(constraints[0].points()[0].normal_impulse, 0.0);
        assert_eq!(constraints[0].points()[0].tangent_impulse, 0.0);
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
    fn the_tangent_is_perpendicular_to_the_normal() {
        let (world, _, _) = scene();

        let constraint = constraints_of(&world, &ImpulseCache::default(), true)[0];

        assert!(constraint.tangent().dot(constraint.normal).abs() < 1e-6);
        assert!((constraint.tangent().length() - 1.0).abs() < 1e-6);
    }
}
