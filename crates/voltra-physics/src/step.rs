//! One fixed step: find what overlaps, then solve it away.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, Transform};

use crate::broad::candidate_pairs;
use crate::integrate::{integrate_positions, integrate_velocities};
use crate::narrow::{manifold, Contact};
use crate::solver::{
    apply_restitution, prepare, solve, warm_start, CachedImpulse, ImpulseCache, SolverBodies,
    SolverParams,
};

/// Advances the world by `dt` and returns the contacts it resolved.
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
pub fn step(
    world: &mut World,
    cache: &mut ImpulseCache,
    params: &SolverParams,
    gravity: Vec2,
    dt: f32,
) -> Vec<Contact> {
    // A step of no time changes nothing, and dividing the sub-step by it would
    // produce an infinite speculative bias. Note the cache is left untouched:
    // a frame that owed no step must not evict contacts that are still there.
    if dt <= 0.0 {
        return Vec::new();
    }

    let contacts = collide(world);
    let mut bodies = SolverBodies::gather(world);

    let sub_steps = params.sub_steps.max(1);
    let h = params.sub_step(dt);
    let mut constraints = prepare(
        &contacts,
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
    cache.commit();

    bodies.scatter(world);

    contacts
}

/// Every overlap in the world right now.
pub(crate) fn collide(world: &World) -> Vec<Contact> {
    candidate_pairs(world)
        .into_iter()
        .filter_map(|(a, b)| {
            let a_shape = (world.get::<Collider>(a)?, world.get::<Transform>(a)?);
            let b_shape = (world.get::<Collider>(b)?, world.get::<Transform>(b)?);
            Some(Contact::new(a, b, manifold(a_shape, b_shape)?))
        })
        .collect()
}
