//! Bodies that come to rest and stacks that stay up: the contact solver
//! measured by where the scene ends up, not by what any pass computed.

mod common;

use common::*;
use voltra_ecs::{Entity, World};
use voltra_physics::{step, ImpulseCache, SolverParams};
use voltra_render::glam::Vec2;
use voltra_scene::{BodyType, Collider, RigidBody, Transform};

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
