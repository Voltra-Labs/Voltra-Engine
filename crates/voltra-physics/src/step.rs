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

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_ecs::Entity;
    use voltra_scene::{BodyType, PhysicsMaterial, RigidBody};

    const DT: f32 = 1.0 / 60.0;
    const G: Vec2 = Vec2::new(0.0, -10.0);

    /// A box, static when `mass` is `None` and dynamic otherwise.
    fn spawn_box(world: &mut World, at: Vec2, half_extents: Vec2, mass: Option<f32>) -> Entity {
        let entity = world.spawn();
        world.insert(entity, Transform::from_translation(at));
        world.insert(entity, Collider::Box { half_extents });
        if let Some(mass) = mass {
            world.insert(entity, RigidBody::new_dynamic(mass));
        }
        entity
    }

    /// A static floor whose top face is at y = -1.5, and a ball of radius 0.5
    /// dropped from the origin. Its resting centre is therefore y = -1.
    fn floor_and_ball() -> (World, Entity, Entity) {
        let mut world = World::new();

        let floor = world.spawn();
        world.insert(floor, Transform::from_translation(Vec2::new(0.0, -2.0)));
        world.insert(
            floor,
            Collider::Box {
                half_extents: Vec2::new(10.0, 0.5),
            },
        );
        world.insert(floor, RigidBody::new_static());

        let ball = world.spawn();
        world.insert(ball, Transform::default());
        world.insert(ball, Collider::Circle { radius: 0.5 });
        world.insert(ball, RigidBody::new_dynamic(1.0));

        (world, floor, ball)
    }

    fn translation(world: &World, entity: Entity) -> Vec2 {
        world
            .get::<Transform>(entity)
            .expect("the transform is there")
            .translation
    }

    fn velocity(world: &World, entity: Entity) -> Vec2 {
        world
            .get::<RigidBody>(entity)
            .expect("the body is there")
            .velocity
    }

    /// Runs `count` steps of the default solver.
    fn run(world: &mut World, cache: &mut ImpulseCache, gravity: Vec2, count: usize) {
        let params = SolverParams::default();
        for _ in 0..count {
            step(world, cache, &params, gravity, DT);
        }
    }

    #[test]
    fn a_falling_body_comes_to_rest_on_the_floor() {
        // Replaces `a_falling_body_keeps_going_through_the_floor`, which pinned
        // 11b-1's stated limit that nothing resolved a contact. Same scene,
        // opposite assertion — the whole point of this stage.
        let (mut world, _, ball) = floor_and_ball();
        let mut cache = ImpulseCache::default();

        run(&mut world, &mut cache, G, 300);

        let y = translation(&world, ball).y;
        assert!((y + 1.0).abs() < 0.05, "it should rest at y = -1, got {y}");
    }

    #[test]
    fn a_body_at_rest_stays_at_rest() {
        let (mut world, _, ball) = floor_and_ball();
        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 300);

        let settled = translation(&world, ball).y;
        run(&mut world, &mut cache, G, 300);

        let y = translation(&world, ball).y;
        assert!(
            (y - settled).abs() < 0.01,
            "it must not creep: {settled} then {y}"
        );
    }

    #[test]
    fn a_stack_of_boxes_settles_without_sinking() {
        let mut world = World::new();
        spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(10.0, 1.0), None);
        let boxes: Vec<Entity> = (0..5)
            .map(|i| {
                spawn_box(
                    &mut world,
                    Vec2::new(0.0, 0.55 + i as f32 * 1.05),
                    Vec2::splat(0.5),
                    Some(1.0),
                )
            })
            .collect();

        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 600);

        // The floor's top face is y = 0 and every box is 1 tall, so the boxes
        // rest at 0.5, 1.5, 2.5 … A little overlap is the soft constraint's
        // slop and is expected; half a box is sinking.
        let mut previous = -0.5;
        for (i, entity) in boxes.iter().enumerate() {
            let y = translation(&world, *entity).y;
            assert!(
                y > previous + 0.95,
                "box {i} sank into what is below it: {y} over {previous}"
            );
            previous = y;
        }
        assert!(previous < 4.6, "the stack must not climb: {previous}");
    }

    #[test]
    fn a_static_body_is_not_pushed_by_what_lands_on_it() {
        let (mut world, floor, _) = floor_and_ball();
        let mut cache = ImpulseCache::default();

        run(&mut world, &mut cache, G, 200);

        assert_eq!(translation(&world, floor), Vec2::new(0.0, -2.0));
    }

    /// A box with a rotation, and a mass if it is dynamic.
    fn spawn_turned_box(
        world: &mut World,
        at: Vec2,
        half_extents: Vec2,
        mass: Option<f32>,
        rotation: f32,
    ) -> Entity {
        let entity = spawn_box(world, at, half_extents, mass);
        world
            .get_mut::<Transform>(entity)
            .expect("just spawned")
            .rotation = rotation;
        entity
    }

    fn rotation(world: &World, entity: Entity) -> f32 {
        world
            .get::<Transform>(entity)
            .expect("the transform is there")
            .rotation
    }

    fn spin(world: &World, entity: Entity) -> f32 {
        world
            .get::<RigidBody>(entity)
            .expect("the body is there")
            .angular_velocity
    }

    #[test]
    fn a_box_dropped_flat_settles_without_rocking() {
        // The reason a manifold carries two points. Given one, the solver
        // corrects a single corner per step, the box tips the other way, and it
        // rocks for as long as the scene runs.
        let mut world = World::new();
        spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(10.0, 1.0), None);
        let crate_ = spawn_box(&mut world, Vec2::new(0.0, 0.6), Vec2::splat(0.5), Some(1.0));

        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 300);

        let angle = rotation(&world, crate_);
        assert!(angle.abs() < 0.01, "it rocked to {angle} rad");
        assert!(
            spin(&world, crate_).abs() < 0.05,
            "{}",
            spin(&world, crate_)
        );
        let y = translation(&world, crate_).y;
        assert!((y - 0.5).abs() < 0.05, "it should rest at y = 0.5, got {y}");
    }

    #[test]
    fn a_box_dropped_on_a_corner_tips_onto_a_face() {
        let mut world = World::new();
        spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(10.0, 1.0), None);
        let crate_ = spawn_turned_box(
            &mut world,
            Vec2::new(0.0, 1.2),
            Vec2::splat(0.5),
            Some(1.0),
            0.5,
        );

        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 600);

        // Tipped back flat: less than half a turn of a quarter turn away from
        // where a face lies on the floor.
        let angle = rotation(&world, crate_);
        assert!(angle.abs() < 0.15, "it stayed on its corner at {angle} rad");
        assert!(spin(&world, crate_).abs() < 0.1, "still rocking");
    }

    #[test]
    fn a_loaded_plank_rotates_about_its_fulcrum() {
        // A plank across a narrow support, weighted on its right end. Nothing
        // about this scene resolves without torque: with 11b-2's solver the
        // plank stays level and the weight sits in the air.
        let mut world = World::new();
        spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(0.2, 1.0), None);
        let plank = spawn_box(
            &mut world,
            Vec2::new(0.0, 0.1),
            Vec2::new(2.0, 0.1),
            Some(1.0),
        );
        spawn_box(&mut world, Vec2::new(1.5, 0.6), Vec2::splat(0.4), Some(5.0));

        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 300);

        // Clockwise: the loaded end is the right one, and y is up.
        assert!(
            rotation(&world, plank) < -0.1,
            "the loaded end must go down, got {}",
            rotation(&world, plank)
        );
    }

    #[test]
    fn an_off_centre_hit_spins_a_body_the_right_way() {
        // A box dropped onto a peg under its left corner. The push is upward
        // and to the left of the centre, so the torque turns it clockwise.
        let mut world = World::new();
        spawn_box(&mut world, Vec2::new(-0.45, -1.0), Vec2::splat(0.1), None);
        let crate_ = spawn_box(&mut world, Vec2::new(0.0, 0.0), Vec2::splat(0.5), Some(1.0));

        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 30);

        assert!(
            spin(&world, crate_) < -0.05,
            "a hit left of centre turns a body clockwise, got {}",
            spin(&world, crate_)
        );
        assert!(
            velocity(&world, crate_).y > -100.0,
            "and it is still finite"
        );
    }

    #[test]
    fn a_locked_body_takes_the_hit_without_turning() {
        let mut world = World::new();
        spawn_box(&mut world, Vec2::new(-0.45, -1.0), Vec2::splat(0.1), None);
        let crate_ = spawn_box(&mut world, Vec2::new(0.0, 0.0), Vec2::splat(0.5), Some(1.0));
        world
            .get_mut::<RigidBody>(crate_)
            .expect("the crate")
            .lock_rotation = true;

        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 30);

        assert_eq!(spin(&world, crate_), 0.0);
        assert_eq!(rotation(&world, crate_), 0.0);
        // And the linear half of the contact still happened.
        assert!(
            velocity(&world, crate_).y > -5.0,
            "the peg must still hold it up: {:?}",
            velocity(&world, crate_)
        );
    }

    /// A static ramp at `angle`, and a box resting on its surface.
    fn ramp(angle: f32, friction: f32) -> (World, Entity) {
        let mut world = World::new();
        let slope = spawn_turned_box(
            &mut world,
            Vec2::new(0.0, -1.0),
            Vec2::new(5.0, 0.5),
            None,
            angle,
        );
        world.insert(
            slope,
            PhysicsMaterial {
                friction,
                restitution: 0.0,
            },
        );

        let up = Vec2::from_angle(angle).rotate(Vec2::new(0.0, 0.98));
        let crate_ = spawn_turned_box(
            &mut world,
            Vec2::new(0.0, -1.0) + up,
            Vec2::splat(0.5),
            Some(1.0),
            angle,
        );
        world.insert(
            crate_,
            PhysicsMaterial {
                friction,
                restitution: 0.0,
            },
        );

        (world, crate_)
    }

    #[test]
    fn a_box_on_a_rough_slope_does_not_slide() {
        // tan(0.3) ≈ 0.31, well under a friction of one.
        let (mut world, crate_) = ramp(0.3, 1.0);
        let start = translation(&world, crate_);

        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 600);

        let moved = translation(&world, crate_) - start;
        assert!(moved.length() < 0.1, "it slid {moved:?}");
    }

    #[test]
    fn a_box_on_a_frictionless_slope_slides_without_spinning() {
        let (mut world, crate_) = ramp(0.3, 0.0);
        let start = translation(&world, crate_);

        let mut cache = ImpulseCache::default();
        // One second. Longer and it runs off the end of the ramp, which says
        // nothing about friction.
        run(&mut world, &mut cache, G, 60);

        // The ramp is turned counter-clockwise, so downhill is -x.
        let moved = translation(&world, crate_) - start;
        assert!(
            moved.x < -0.5 && moved.y < 0.0,
            "it should slide down the slope, got {moved:?}"
        );
        // A box on a plane has no reason to turn without friction, and one that
        // does is a sign the two contact points are being pushed unequally.
        assert!(
            spin(&world, crate_).abs() < 0.2,
            "it spun at {}",
            spin(&world, crate_)
        );
    }

    #[test]
    fn a_stack_of_rotated_boxes_does_not_gain_energy() {
        let mut world = World::new();
        spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(10.0, 1.0), None);
        let boxes: Vec<Entity> = (0..4)
            .map(|i| {
                spawn_turned_box(
                    &mut world,
                    Vec2::new(0.0, 0.55 + i as f32 * 1.05),
                    Vec2::splat(0.5),
                    Some(1.0),
                    0.05 * (i as f32 - 1.5),
                )
            })
            .collect();

        let energy = |world: &World| -> f32 {
            boxes
                .iter()
                .map(|entity| {
                    let body = world.get::<RigidBody>(*entity).expect("a body");
                    body.velocity.length_squared() + body.angular_velocity * body.angular_velocity
                })
                .sum()
        };

        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 10);
        let early = energy(&world);
        run(&mut world, &mut cache, G, 300);

        assert!(
            energy(&world) <= early + 1e-3,
            "energy grew from {early} to {}",
            energy(&world)
        );
    }

    #[test]
    fn a_body_spun_absurdly_fast_stays_in_the_world() {
        let mut world = World::new();
        spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(10.0, 1.0), None);
        let crate_ = spawn_box(&mut world, Vec2::new(0.0, 0.6), Vec2::splat(0.5), Some(1.0));
        world
            .get_mut::<RigidBody>(crate_)
            .expect("the crate")
            .angular_velocity = 1.0e6;

        let mut cache = ImpulseCache::default();
        // Half a second: a box spinning at the cap rubs against the floor and
        // is fired sideways, which is what friction on a wheel does. Run it
        // longer and it leaves the end of the floor, which is not the point.
        run(&mut world, &mut cache, G, 30);

        let position = translation(&world, crate_);
        assert!(position.is_finite(), "{position:?}");
        assert!(rotation(&world, crate_).is_finite());
        assert!(
            position.y > 0.0 && position.x.abs() < 9.0,
            "it must stay on top of the floor, not through it: {position:?}"
        );
    }

    #[test]
    fn a_kinematic_body_is_not_pushed_by_what_lands_on_it() {
        let mut world = World::new();
        let platform = world.spawn();
        world.insert(platform, Transform::from_translation(Vec2::new(0.0, -2.0)));
        world.insert(
            platform,
            Collider::Box {
                half_extents: Vec2::new(10.0, 0.5),
            },
        );
        world.insert(
            platform,
            RigidBody {
                body_type: BodyType::Kinematic,
                ..Default::default()
            },
        );
        let ball = spawn_box(&mut world, Vec2::ZERO, Vec2::splat(0.5), Some(1.0));

        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 300);

        assert_eq!(translation(&world, platform).y, -2.0);
        assert!(translation(&world, ball).y > -1.6, "the ball still stops");
    }

    #[test]
    fn friction_stops_a_sliding_body() {
        let mut world = World::new();
        spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(20.0, 1.0), None);
        let crate_ = spawn_box(
            &mut world,
            Vec2::new(0.0, 0.45),
            Vec2::splat(0.5),
            Some(1.0),
        );
        world.get_mut::<RigidBody>(crate_).expect("body").velocity = Vec2::new(10.0, 0.0);

        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 600);

        let speed = velocity(&world, crate_).x;
        assert!(speed.abs() < 0.5, "friction should stop it, got {speed}");
    }

    #[test]
    fn a_frictionless_body_keeps_sliding() {
        let mut world = World::new();
        let floor = spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(20.0, 1.0), None);
        let slippery = PhysicsMaterial {
            friction: 0.0,
            restitution: 0.0,
        };
        world.insert(floor, slippery);
        let puck = spawn_box(
            &mut world,
            Vec2::new(0.0, 0.45),
            Vec2::splat(0.5),
            Some(1.0),
        );
        world.insert(puck, slippery);
        world.get_mut::<RigidBody>(puck).expect("body").velocity = Vec2::new(10.0, 0.0);

        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 300);

        let speed = velocity(&world, puck).x;
        assert!(speed > 9.5, "nothing should slow it, got {speed}");
    }

    #[test]
    fn a_bouncy_body_returns_most_of_its_drop() {
        let mut world = World::new();
        let floor = spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(20.0, 1.0), None);
        let bouncy = PhysicsMaterial {
            friction: 0.6,
            restitution: 0.9,
        };
        world.insert(floor, bouncy);
        let ball = spawn_box(&mut world, Vec2::new(0.0, 5.0), Vec2::splat(0.5), Some(1.0));
        world.insert(ball, bouncy);

        let mut cache = ImpulseCache::default();
        let params = SolverParams::default();
        let mut bounced = false;
        let mut highest: f32 = -10.0;
        for _ in 0..600 {
            step(&mut world, &mut cache, &params, G, DT);
            if velocity(&world, ball).y > 1.0 {
                bounced = true;
            }
            if bounced {
                highest = highest.max(translation(&world, ball).y);
            }
        }

        assert!(bounced, "restitution 0.9 must send it back up");
        assert!(
            highest > 2.0,
            "it should return most of the drop: {highest}"
        );
    }

    #[test]
    fn a_dull_body_does_not_bounce() {
        let mut world = World::new();
        spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(20.0, 1.0), None);
        let ball = spawn_box(&mut world, Vec2::new(0.0, 5.0), Vec2::splat(0.5), Some(1.0));

        let mut cache = ImpulseCache::default();
        let params = SolverParams::default();
        let mut highest_after_landing: f32 = -10.0;
        let mut landed = false;
        for _ in 0..600 {
            step(&mut world, &mut cache, &params, G, DT);
            if translation(&world, ball).y < 0.6 {
                landed = true;
            }
            if landed {
                highest_after_landing = highest_after_landing.max(translation(&world, ball).y);
            }
        }

        assert!(landed);
        assert!(
            highest_after_landing < 0.7,
            "restitution 0 must not bounce: {highest_after_landing}"
        );
    }

    #[test]
    fn a_resting_body_does_not_bounce_on_its_own_noise() {
        // Restitution below the threshold speed is discarded. Without that, a
        // perfectly elastic body vibrates on the floor forever.
        let mut world = World::new();
        let floor = spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(20.0, 1.0), None);
        let elastic = PhysicsMaterial {
            friction: 0.6,
            restitution: 1.0,
        };
        world.insert(floor, elastic);
        let ball = spawn_box(
            &mut world,
            Vec2::new(0.0, 0.49),
            Vec2::splat(0.5),
            Some(1.0),
        );
        world.insert(ball, elastic);

        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 600);

        let speed = velocity(&world, ball).y;
        assert!(speed.abs() < 1.0, "it must settle, got {speed}");
    }

    #[test]
    fn a_hundred_to_one_mass_ratio_still_stacks() {
        // The ratio a soft solver is expected to hold. Beyond it a light body
        // is squeezed out sideways rather than crushed — see the test below,
        // which pins that it at least stays finite.
        let mut world = World::new();
        spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(10.0, 1.0), None);
        let light = spawn_box(
            &mut world,
            Vec2::new(0.0, 0.55),
            Vec2::splat(0.5),
            Some(0.01),
        );
        let heavy = spawn_box(&mut world, Vec2::new(0.0, 1.6), Vec2::splat(0.5), Some(1.0));

        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 600);

        let (light_y, heavy_y) = (translation(&world, light).y, translation(&world, heavy).y);
        assert!(heavy_y > light_y, "light {light_y} heavy {heavy_y}");
        assert!(
            light_y > 0.4,
            "the light box must not be crushed: {light_y}"
        );
    }

    #[test]
    fn a_thousand_to_one_mass_ratio_does_not_explode() {
        // Past roughly a hundred to one, the light box is extruded sideways:
        // squeezed hard enough that the axis of least penetration turns from
        // vertical to horizontal, and out it goes. Every impulse solver has a
        // mass-ratio limit and this is ours. What must still hold is that
        // nothing becomes a NaN and nothing is launched out of the scene.
        let mut world = World::new();
        spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(10.0, 1.0), None);
        let light = spawn_box(
            &mut world,
            Vec2::new(0.0, 0.55),
            Vec2::splat(0.5),
            Some(0.001),
        );
        let heavy = spawn_box(&mut world, Vec2::new(0.0, 1.6), Vec2::splat(0.5), Some(1.0));

        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 600);

        assert!(translation(&world, light).is_finite());
        assert!(translation(&world, heavy).is_finite());
        assert!(
            translation(&world, light).length() < 10.0,
            "the light box must be extruded, not fired: {:?}",
            translation(&world, light)
        );
        assert!(
            translation(&world, heavy).y < 10.0,
            "the light box must not launch the heavy one"
        );
    }

    #[test]
    fn the_cache_forgets_a_body_that_was_despawned() {
        let (mut world, _, ball) = floor_and_ball();
        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 200);
        assert!(!cache.is_empty(), "a resting body should be warm started");

        world.despawn(ball);
        run(&mut world, &mut cache, G, 1);

        assert!(
            cache.is_empty(),
            "the pair is gone, so its impulses must be"
        );
    }

    #[test]
    fn the_cache_forgets_a_pair_that_separated() {
        let mut world = World::new();
        spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(10.0, 1.0), None);
        let ball = spawn_box(&mut world, Vec2::new(0.0, 0.5), Vec2::splat(0.5), Some(1.0));

        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, G, 60);
        assert!(!cache.is_empty());

        world
            .get_mut::<Transform>(ball)
            .expect("transform")
            .translation = Vec2::new(0.0, 50.0);
        run(&mut world, &mut cache, Vec2::ZERO, 1);

        assert!(cache.is_empty());
    }

    #[test]
    fn coincident_bodies_separate_without_a_nan() {
        let mut world = World::new();
        let a = spawn_box(&mut world, Vec2::ZERO, Vec2::splat(0.5), Some(1.0));
        let b = spawn_box(&mut world, Vec2::ZERO, Vec2::splat(0.5), Some(1.0));

        let mut cache = ImpulseCache::default();
        run(&mut world, &mut cache, Vec2::ZERO, 240);

        let (a, b) = (translation(&world, a), translation(&world, b));
        assert!(a.is_finite() && b.is_finite(), "{a:?} {b:?}");
        assert!(a.distance(b) > 0.5, "they must separate: {a:?} {b:?}");
    }

    #[test]
    fn a_falling_body_eventually_reaches_the_floor() {
        let (mut world, _, _) = floor_and_ball();
        let mut cache = ImpulseCache::default();
        let params = SolverParams::default();

        let mut contacts = Vec::new();
        for _ in 0..240 {
            contacts = step(&mut world, &mut cache, &params, G, DT);
            if !contacts.is_empty() {
                break;
            }
        }

        assert_eq!(contacts.len(), 1, "one contact, between ball and floor");
        assert!(
            contacts[0].normal().y.abs() > 0.5,
            "the contact must separate them vertically, got {:?}",
            contacts[0].normal()
        );
    }

    #[test]
    fn an_empty_world_steps_without_contacts() {
        let mut world = World::new();
        let mut cache = ImpulseCache::default();

        assert!(step(&mut world, &mut cache, &SolverParams::default(), G, DT).is_empty());
    }

    #[test]
    fn a_zero_length_step_changes_nothing() {
        let (mut world, _, ball) = floor_and_ball();
        let before = translation(&world, ball);
        let mut cache = ImpulseCache::default();

        let contacts = step(&mut world, &mut cache, &SolverParams::default(), G, 0.0);

        assert!(contacts.is_empty());
        assert_eq!(translation(&world, ball), before);
    }

    #[test]
    fn a_body_with_no_collider_moves_and_never_collides() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform::default());
        world.insert(entity, RigidBody::new_dynamic(1.0));

        let mut cache = ImpulseCache::default();
        let contacts = step(&mut world, &mut cache, &SolverParams::default(), G, DT);

        assert!(contacts.is_empty());
        assert!(translation(&world, entity).y < 0.0);
    }

    #[test]
    fn a_collider_with_no_body_is_static_geometry() {
        let mut world = World::new();
        let wall = world.spawn();
        world.insert(wall, Transform::default());
        world.insert(
            wall,
            Collider::Box {
                half_extents: Vec2::splat(1.0),
            },
        );

        let ball = world.spawn();
        world.insert(ball, Transform::from_translation(Vec2::new(0.5, 0.0)));
        world.insert(ball, Collider::Circle { radius: 0.5 });

        let mut cache = ImpulseCache::default();
        let contacts = step(&mut world, &mut cache, &SolverParams::default(), G, DT);

        assert_eq!(contacts.len(), 1);
        assert_eq!(
            translation(&world, wall),
            Vec2::ZERO,
            "a collider with no body must not move"
        );
    }

    #[test]
    fn the_contact_names_the_entities_it_is_between() {
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Transform::default());
        world.insert(a, Collider::Circle { radius: 1.0 });
        let b = world.spawn();
        world.insert(b, Transform::from_translation(Vec2::new(1.0, 0.0)));
        world.insert(b, Collider::Circle { radius: 1.0 });

        let mut cache = ImpulseCache::default();
        let contacts = step(&mut world, &mut cache, &SolverParams::default(), G, DT);

        assert_eq!(contacts.len(), 1);
        assert_eq!((contacts[0].a, contacts[0].b), (a, b));
    }

    #[test]
    fn a_scene_of_immovable_shapes_reports_the_same_contacts_every_step() {
        // Detection must be a function of the state, not of how many times it
        // has been called, and two static shapes give the solver nothing to
        // change.
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Transform::default());
        world.insert(a, Collider::Circle { radius: 1.0 });
        let b = world.spawn();
        world.insert(b, Transform::from_translation(Vec2::new(1.0, 0.0)));
        world.insert(b, Collider::Circle { radius: 1.0 });

        let mut cache = ImpulseCache::default();
        let params = SolverParams::default();
        let first = step(&mut world, &mut cache, &params, Vec2::ZERO, DT);
        let second = step(&mut world, &mut cache, &params, Vec2::ZERO, DT);

        assert_eq!(first, second);
    }

    #[test]
    fn warm_starting_switched_off_settles_worse() {
        // The demonstration the switch exists for: without last step's
        // impulses, the stack rediscovers them each step and sags.
        let stack = |warm_starting: bool| {
            let mut world = World::new();
            spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(10.0, 1.0), None);
            let boxes: Vec<Entity> = (0..5)
                .map(|i| {
                    spawn_box(
                        &mut world,
                        Vec2::new(0.0, 0.55 + i as f32 * 1.05),
                        Vec2::splat(0.5),
                        Some(1.0),
                    )
                })
                .collect();

            let mut cache = ImpulseCache::default();
            let params = SolverParams {
                warm_starting,
                ..Default::default()
            };
            for _ in 0..600 {
                step(&mut world, &mut cache, &params, G, DT);
            }
            translation(&world, boxes[4]).y
        };

        assert!(stack(true) > stack(false));
    }
}
