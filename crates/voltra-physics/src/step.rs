//! One fixed step: find what overlaps, then solve it away.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, Sensor, Transform};

use crate::broad::candidate_pairs;
use crate::integrate::{integrate_positions, integrate_velocities};
use crate::narrow::{manifold, Contact};
use crate::sleep::Sleep;
use crate::solver::{
    apply_restitution, prepare, solve, warm_start, CachedImpulse, ImpulseCache, SolverBodies,
    SolverParams,
};

/// What a collide found: the contacts to solve, and the sensor pairs to
/// report and nothing else.
///
/// Split here rather than filtered later so a sensor never reaches the solver,
/// never enters the impulse cache and never appears in
/// [`PhysicsWorld::contacts`] — which stays "what the solver resolved", so a
/// character cannot stand on a trigger and a ground check needs no idea that
/// sensors exist.
///
/// [`PhysicsWorld::contacts`]: crate::PhysicsWorld::contacts
#[derive(Debug, Default, Clone)]
pub struct Overlaps {
    /// Solid overlaps, in the order the broad phase found them.
    pub contacts: Vec<Contact>,
    /// Overlaps where at least one side is a [`Sensor`].
    pub sensors: Vec<(Entity, Entity)>,
}

impl Overlaps {
    /// Every overlapping pair and whether it is a sensor pair, which is what
    /// [`Touching::update`] diffs into events.
    ///
    /// [`Touching::update`]: crate::events::Touching::update
    pub fn pairs(&self) -> impl Iterator<Item = (Entity, Entity, bool)> + '_ {
        self.contacts
            .iter()
            .map(|contact| (contact.a, contact.b, false))
            .chain(self.sensors.iter().map(|&(a, b)| (a, b, true)))
    }
}

/// Advances the world by `dt` and returns what it found overlapping.
///
/// The order is TGS Soft's, and each part of it is load-bearing:
///
/// 1. **Collide once**, from the positions the step starts at. The normal is
///    then held constant and each contact's separation is tracked from how far
///    the bodies move, so the narrow phase runs once rather than per sub-step.
/// 2. **Prepare and warm start**, seeding every contact with the impulse it
///    needed last step.
/// 3. **Sub-step**: integrate velocities, solve the contacts with the soft bias
///    pushing overlaps apart, integrate positions. Four short steps converge
///    where four passes over one long step do not.
/// 4. **Relax**: solve once more with the bias off and positions frozen, which
///    removes the energy the bias added. Friction is applied here.
/// 5. **Restitution**, then store the impulses for the next step.
///
/// The contacts returned describe the state at the *start* of the step, which
/// is what the debug overlay draws — the same one-step lag Box2D has.
///
/// Every body is simulated: there is no sleep state to carry between calls.
/// [`step_sleeping`] is the same step with one, and is what
/// [`PhysicsWorld`](crate::PhysicsWorld) runs.
pub fn step(
    world: &mut World,
    cache: &mut ImpulseCache,
    params: &SolverParams,
    gravity: Vec2,
    dt: f32,
) -> Overlaps {
    step_sleeping(world, cache, params, gravity, dt, &mut Sleep::default())
}

/// [`step`], with bodies at rest put to sleep and left out of the solve
/// until something disturbs them — see [`crate::sleep`].
///
/// The overlaps returned still include sleeping contacts: a resting
/// character is still standing on the floor, and an event for a pair that
/// went to sleep touching must not say it ended.
pub fn step_sleeping(
    world: &mut World,
    cache: &mut ImpulseCache,
    params: &SolverParams,
    gravity: Vec2,
    dt: f32,
    sleep: &mut Sleep,
) -> Overlaps {
    // A step of no time changes nothing, and dividing the sub-step by it would
    // produce an infinite speculative bias. Note the cache is left untouched:
    // a frame that owed no step must not evict contacts that are still there.
    if dt <= 0.0 {
        return Overlaps::default();
    }

    sleep.wake_disturbed(world);
    let overlaps = collide_with(world, |a, b| sleep.resting_contact(world, a, b).cloned());
    sleep.wake_islands(world, &overlaps.contacts);

    // Only contacts with an awake dynamic body are solved. After the wake
    // above, a contact touching a sleeping body has nothing awake on the
    // other side either, so the pair is at rest and its impulses are carried
    // over untouched for when it wakes.
    let mut solved: Vec<Contact> = Vec::with_capacity(overlaps.contacts.len());
    let mut resting_pairs = crate::hash::FastSet::default();
    for contact in &overlaps.contacts {
        if sleep.is_awake_dynamic(world, contact.a) || sleep.is_awake_dynamic(world, contact.b) {
            solved.push(*contact);
        } else {
            resting_pairs.insert((contact.a, contact.b));
            sleep.remember_resting(contact);
        }
    }
    sleep.keep_resting(&resting_pairs);
    let contacts = &solved;

    let mut bodies = SolverBodies::gather(world);
    for entity in sleep.asleep() {
        bodies.freeze(entity);
    }

    let sub_steps = params.sub_steps.max(1);
    let h = params.sub_step(dt);
    let mut constraints = prepare(
        contacts,
        &bodies,
        world,
        params.softness(h),
        cache,
        params.warm_starting,
    );

    warm_start(&constraints, &mut bodies);

    for _ in 0..sub_steps {
        integrate_velocities(&mut bodies, gravity, h, params.max_rotation);
        solve(
            &mut constraints,
            &mut bodies,
            h,
            true,
            params.max_push_speed,
        );
        integrate_positions(&mut bodies, h);
    }

    // Relaxation: the same solve without the bias, and deliberately without
    // integrating positions afterwards.
    solve(
        &mut constraints,
        &mut bodies,
        h,
        false,
        params.max_push_speed,
    );
    apply_restitution(&mut constraints, &mut bodies, params.restitution_threshold);

    for constraint in &constraints {
        for point in constraint.points() {
            cache.record(
                constraint.key_of(point),
                CachedImpulse {
                    normal: point.normal_impulse,
                    tangent: point.tangent_impulse,
                },
            );
        }
    }
    cache.keep(|a, b| resting_pairs.contains(&(a, b)));
    cache.commit();

    bodies.scatter(world);
    sleep.settle(world, contacts, &params.sleep, dt);

    overlaps
}

/// Every overlap in the world right now, sensors kept apart from the rest.
#[cfg(test)]
pub(crate) fn collide(world: &World) -> Overlaps {
    collide_with(world, |_, _| None)
}

/// [`collide`], taking `reuse`'s contact for a pair instead of testing it
/// when it has one — how sleeping pairs skip the narrow phase.
fn collide_with(world: &World, reuse: impl Fn(Entity, Entity) -> Option<Contact>) -> Overlaps {
    let mut overlaps = Overlaps::default();
    for (a, b) in candidate_pairs(world) {
        if let Some(contact) = reuse(a, b) {
            overlaps.contacts.push(contact);
            continue;
        }
        let Some(a_shape) = shape(world, a) else {
            continue;
        };
        let Some(b_shape) = shape(world, b) else {
            continue;
        };
        let Some(manifold) = manifold(a_shape, b_shape) else {
            continue;
        };

        // Either side being a sensor makes the pair one: a trigger volume must
        // not push the player out, and a player must not be stopped by it.
        if world.get::<Sensor>(a).is_some() || world.get::<Sensor>(b).is_some() {
            overlaps.sensors.push((a, b));
        } else {
            overlaps.contacts.push(Contact::new(a, b, manifold));
        }
    }
    overlaps
}

/// The shape `entity` collides with, if it has one and sits somewhere.
fn shape(world: &World, entity: Entity) -> Option<(&Collider, &Transform)> {
    Some((
        world.get::<Collider>(entity)?,
        world.get::<Transform>(entity)?,
    ))
}
