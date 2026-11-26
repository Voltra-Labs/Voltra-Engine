//! One fixed step: find what overlaps, then solve it away.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use crate::integrate::{integrate_positions, integrate_velocities};
use crate::narrow::Contact;
use crate::one_way::OneWayPasses;
use crate::sleep::Sleep;
use crate::solver::{
    apply_restitution, joint_ends, prepare, prepare_joints, solve, solve_joints, store_joints,
    warm_start, warm_start_joints, CachedImpulse, ImpulseCache, SolverBodies, SolverParams,
};

mod collide;
mod overlaps;
#[cfg(test)]
pub(crate) use collide::collide;
use collide::{collide_with, ordered, unjoined, Reuse};
pub use overlaps::Overlaps;

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
///    where four passes over one long step do not. Joints go first in each
///    sub-step and, as Box2D v3 has them, are warm started and relaxed in it
///    too (see [`crate::solver::joint`]).
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
    step_sleeping(
        world,
        cache,
        params,
        gravity,
        dt,
        &mut Sleep::default(),
        &mut OneWayPasses::default(),
    )
}

/// [`step`], with bodies at rest put to sleep and left out of the solve
/// until something disturbs them — see [`crate::sleep`] — and the pairs a
/// one-way collider is letting through remembered — see
/// [`crate::one_way`].
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
    passes: &mut OneWayPasses,
) -> Overlaps {
    // A step of no time changes nothing, and dividing the sub-step by it would
    // produce an infinite speculative bias. Note the cache is left untouched:
    // a frame that owed no step must not evict contacts that are still there.
    if dt <= 0.0 {
        return Overlaps::default();
    }

    let collide = voltra_profile::scope("collide");
    sleep.wake_disturbed(world);
    let joints = joint_ends(world);
    let apart = unjoined(&joints);
    let overlaps = collide_with(
        world,
        |a, b| {
            if apart.contains(&ordered(a, b)) {
                return Reuse::Skip;
            }
            match sleep.resting_contact(world, a, b) {
                Some(contact) => Reuse::Contact(*contact),
                None => Reuse::Test,
            }
        },
        passes,
    );
    drop(collide);
    let islands = voltra_profile::scope("islands");
    // A joint links its two bodies as a contact does: one waking wakes the
    // other, and they rest together or not at all.
    let joined: Vec<(Entity, Entity)> = joints
        .iter()
        .filter_map(|joint| joint.b.map(|b| (joint.a, b)))
        .collect();
    let mut links: Vec<(Entity, Entity)> = overlaps
        .contacts
        .iter()
        .map(|contact| (contact.a, contact.b))
        .collect();
    links.extend_from_slice(&joined);
    sleep.wake_islands(world, &links);

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
    if params.continuous {
        let _continuous = voltra_profile::scope("continuous");
        let touching: crate::hash::FastSet<(Entity, Entity)> = overlaps
            .contacts
            .iter()
            .map(|contact| ordered(contact.a, contact.b))
            .collect();
        // Solved, never reported: a contact about to happen is not one yet.
        solved.extend(crate::continuous::speculative_contacts(
            world, sleep, gravity, dt, passes, &apart, &touching,
        ));
    }
    let contacts = &solved;
    voltra_profile::count("contacts", overlaps.contacts.len() as f64);
    voltra_profile::count("contacts solved", solved.len() as f64);
    drop(islands);
    let solve_scope = voltra_profile::scope("solve");

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
    let mut joint_constraints = prepare_joints(world, &bodies, &joints, h);
    let joint_softness = params.joint_softness(h);

    warm_start(&constraints, &mut bodies);

    for _ in 0..sub_steps {
        integrate_velocities(&mut bodies, gravity, h, params.max_rotation);
        // Joints as Box2D v3 solves them: warm started, solved and relaxed in
        // every sub-step, so the impulse they carry is one sub-step's — which
        // is what their springs' `impulse_scale` is tuned against.
        if params.warm_starting {
            warm_start_joints(&joint_constraints, &mut bodies);
        }
        solve_joints(&mut joint_constraints, &mut bodies, joint_softness, h, true);
        solve(
            &mut constraints,
            &mut bodies,
            h,
            true,
            params.max_push_speed,
        );
        integrate_positions(&mut bodies, h);
        solve_joints(
            &mut joint_constraints,
            &mut bodies,
            joint_softness,
            h,
            false,
        );
    }

    // Relaxation: the same solve without the bias, and deliberately without
    // integrating positions afterwards.
    solve_joints(
        &mut joint_constraints,
        &mut bodies,
        joint_softness,
        h,
        false,
    );
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

    drop(solve_scope);
    if params.continuous {
        let _continuous = voltra_profile::scope("continuous");
        crate::continuous::sweep(world, &mut bodies, passes, &apart);
    }
    let _settle = voltra_profile::scope("settle");
    bodies.scatter(world);
    store_joints(&joint_constraints, world);
    let mut settled: Vec<(Entity, Entity)> = contacts
        .iter()
        .map(|contact| (contact.a, contact.b))
        .collect();
    settled.extend_from_slice(&joined);
    let driven: Vec<Entity> = joints
        .iter()
        .filter(|joint| joint.driven)
        .flat_map(|joint| std::iter::once(joint.a).chain(joint.b))
        .collect();
    sleep.settle(world, &settled, &driven, &params.sleep, dt);

    overlaps
}
