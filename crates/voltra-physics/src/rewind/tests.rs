//! A target moving along x one unit a tick, recorded, and asked about.

use voltra_render::glam::Vec2;
use voltra_scene::{Collider, LagCompensated, Transform};

use super::*;
use crate::query::{self, QueryFilter};

/// A ray straight up through `x`, from below the floor of everything.
fn up_at(rewound: &Rewound<'_>, x: f32) -> Option<Entity> {
    rewound
        .ray(Vec2::new(x, -10.0), Vec2::Y, 20.0, QueryFilter::new())
        .map(|hit| hit.entity)
}

fn target(world: &mut World, x: f32) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform::from_translation(Vec2::new(x, 0.0)));
    world.insert(
        entity,
        Collider::Box {
            half_extents: Vec2::splat(0.25),
        },
    );
    world.insert(entity, LagCompensated);
    entity
}

fn move_to(world: &mut World, entity: Entity, x: f32) {
    if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.translation.x = x;
    }
}

/// A target at x = tick for ticks 1..=10, recorded each tick.
fn moving() -> (World, Entity, PoseHistory) {
    let mut world = World::new();
    let entity = target(&mut world, 0.0);
    let mut history = PoseHistory::new(60);
    for tick in 1..=10 {
        move_to(&mut world, entity, tick as f32);
        history.record(&world, tick);
    }
    (world, entity, history)
}

#[test]
fn a_ray_hits_the_past_pose_and_misses_the_present_one() {
    let (world, entity, history) = moving();
    let past = history.rewound(&world, 4.0);
    assert_eq!(up_at(&past, 4.0), Some(entity), "where it was at tick 4");
    assert_eq!(up_at(&past, 10.0), None, "not where it is now");
    // And the live world the other way round.
    let now = Rewound::live(&world);
    assert_eq!(up_at(&now, 4.0), None);
    assert_eq!(up_at(&now, 10.0), Some(entity));
    // The world itself was never moved.
    assert_eq!(
        world.get::<Transform>(entity).map(|t| t.translation.x),
        Some(10.0)
    );
}

#[test]
fn a_moment_between_two_ticks_is_interpolated() {
    let (world, entity, history) = moving();
    let between = history.rewound(&world, 6.5);
    assert_eq!(between.tick(), Some(6.5));
    let at = between.pose(entity).expect("it was there");
    assert!((at.translation.x - 6.5).abs() < 1e-6, "{at:?}");
    // 6.5 ± the half-width of 0.25 is covered; neither tick's own pose.
    assert_eq!(up_at(&between, 6.5), Some(entity));
    assert_eq!(up_at(&between, 6.0), None);
    assert_eq!(up_at(&between, 7.0), None);
}

#[test]
fn an_entity_spawned_after_the_moment_is_not_there() {
    let (mut world, _, mut history) = moving();
    let late = target(&mut world, 20.0);
    history.record(&world, 11);
    let before = history.rewound(&world, 9.0);
    assert_eq!(up_at(&before, 20.0), None, "not yet spawned at tick 9");
    assert_eq!(before.pose(late), None);
    // Between its spawn tick and the one before: still not there.
    assert_eq!(up_at(&history.rewound(&world, 10.5), 20.0), None);
    // Unrecorded yet, and asked about at the newest: the world as it is.
    let fresh = target(&mut world, 30.0);
    assert_eq!(up_at(&history.rewound(&world, 11.0), 30.0), Some(fresh));
    assert_eq!(up_at(&history.rewound(&world, 10.0), 30.0), None);
}

#[test]
fn an_entity_despawned_in_the_window_is_not_hit() {
    let (mut world, entity, history) = moving();
    world.despawn(entity);
    let past = history.rewound(&world, 4.0);
    assert_eq!(up_at(&past, 4.0), None, "gone is gone");
    assert_eq!(past.pose(entity), None);
}

#[test]
fn the_ring_keeps_its_capacity_and_clamps_to_its_oldest() {
    let mut world = World::new();
    let entity = target(&mut world, 0.0);
    let mut history = PoseHistory::new(4);
    for tick in 1..=10 {
        move_to(&mut world, entity, tick as f32);
        history.record(&world, tick);
    }
    assert_eq!(history.len(), 4);
    assert_eq!((history.oldest(), history.newest()), (Some(7), Some(10)));
    // Older than anything held: the oldest.
    let far = history.rewound(&world, 2.0);
    assert_eq!(far.tick(), Some(7.0));
    assert_eq!(up_at(&far, 7.0), Some(entity));
    assert_eq!(up_at(&far, 2.0), None);
    // A covering ring: a second at 60 a second, and now.
    assert_eq!(
        PoseHistory::covering(Duration::from_secs(1), 1.0 / 60.0).capacity(),
        61
    );
}

#[test]
fn the_newest_tick_or_later_and_an_empty_history_are_the_live_world() {
    let (world, entity, history) = moving();
    for tick in [10.0, 10.2, 1e9, f64::NAN] {
        let rewound = history.rewound(&world, tick);
        assert_eq!(rewound.tick(), None, "{tick}");
        assert_eq!(up_at(&rewound, 10.0), Some(entity));
    }
    let empty = PoseHistory::new(8);
    let live = empty.rewound(&world, 3.0);
    assert_eq!(live.tick(), None);
    assert_eq!(up_at(&live, 10.0), Some(entity));
    assert_eq!(
        live.ray(Vec2::new(10.0, -10.0), Vec2::Y, 20.0, QueryFilter::new()),
        query::ray(
            &world,
            Vec2::new(10.0, -10.0),
            Vec2::Y,
            20.0,
            QueryFilter::new()
        )
    );
}

#[test]
fn an_entity_not_marked_is_queried_where_it_is() {
    let (mut world, _, history) = moving();
    let wall = world.spawn();
    world.insert(wall, Transform::from_translation(Vec2::new(-5.0, 0.0)));
    world.insert(wall, Collider::default());
    let past = history.rewound(&world, 3.0);
    assert_eq!(up_at(&past, -5.0), Some(wall));
    assert!(past.compensated().all(|e| e != wall));
}

#[test]
fn every_query_reads_the_past() {
    let (world, entity, history) = moving();
    let past = history.rewound(&world, 3.0);
    let filter = QueryFilter::new();
    assert_eq!(past.point(Vec2::new(3.0, 0.0), filter), Some(entity));
    assert_eq!(past.point(Vec2::new(10.0, 0.0), filter), None);
    assert_eq!(
        past.overlap_aabb(Vec2::new(2.5, -1.0), Vec2::new(3.5, 1.0), filter),
        vec![entity]
    );
    let probe = Collider::Circle { radius: 0.1 };
    let hit = past
        .cast(
            &probe,
            &Transform::from_translation(Vec2::new(3.0, -5.0)),
            Vec2::new(0.0, 10.0),
            filter,
        )
        .expect("met on the way up");
    assert_eq!(hit.entity, entity);
}

#[test]
fn a_hidden_entity_is_found_by_nothing() {
    let (world, entity, history) = moving();
    let mut past = history.rewound(&world, 3.0);
    past.hide(entity);
    assert_eq!(up_at(&past, 3.0), None);
    let mut now = Rewound::live(&world);
    now.hide(entity);
    assert_eq!(up_at(&now, 10.0), None);
}

#[test]
fn the_shape_is_the_one_it_had() {
    let (mut world, entity, mut history) = moving();
    // Crouched from tick 11 on: half as tall, so a ray at y = 0.2 passes
    // over it now and met it before.
    world.insert(
        entity,
        Collider::Box {
            half_extents: Vec2::new(0.25, 0.1),
        },
    );
    history.record(&world, 11);
    let across = |rewound: &Rewound<'_>| {
        rewound
            .ray(Vec2::new(0.0, 0.2), Vec2::X, 20.0, QueryFilter::new())
            .map(|hit| hit.entity)
    };
    assert_eq!(across(&history.rewound(&world, 10.0)), Some(entity));
    assert_eq!(across(&Rewound::live(&world)), None);
}

#[test]
fn a_tick_that_goes_back_starts_again() {
    let (world, _, mut history) = moving();
    history.record(&world, 3);
    assert_eq!(history.len(), 1);
    assert_eq!(history.oldest(), Some(3));
}
