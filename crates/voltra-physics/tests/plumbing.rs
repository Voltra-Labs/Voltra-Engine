//! The step as bookkeeping: what it reports, what the cache forgets, and the
//! degenerate scenes it must survive.

mod common;

use common::*;
use voltra_ecs::World;
use voltra_physics::{step, ImpulseCache, SolverParams};
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, RigidBody, Transform};

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
        contacts = step(&mut world, &mut cache, &params, G, DT).contacts;
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

    assert!(
        step(&mut world, &mut cache, &SolverParams::default(), G, DT)
            .contacts
            .is_empty()
    );
}

#[test]
fn a_zero_length_step_changes_nothing() {
    let (mut world, _, ball) = floor_and_ball();
    let before = translation(&world, ball);
    let mut cache = ImpulseCache::default();

    let contacts = step(&mut world, &mut cache, &SolverParams::default(), G, 0.0).contacts;

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
    let contacts = step(&mut world, &mut cache, &SolverParams::default(), G, DT).contacts;

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
    let contacts = step(&mut world, &mut cache, &SolverParams::default(), G, DT).contacts;

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
    let contacts = step(&mut world, &mut cache, &SolverParams::default(), G, DT).contacts;

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
    let first = step(&mut world, &mut cache, &params, Vec2::ZERO, DT).contacts;
    let second = step(&mut world, &mut cache, &params, Vec2::ZERO, DT).contacts;

    assert_eq!(first, second);
}
