//! Rays, points and boxes asked of a world of a few shapes.

use super::*;
use voltra_scene::{CollisionLayers, Sensor};

const NEAR: f32 = 1e-4;

fn box_at(world: &mut World, at: Vec2, half: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform2D::from_translation(at));
    world.insert(entity, Collider2D::Box { half_extents: half });
    entity
}

fn circle_at(world: &mut World, at: Vec2, radius: f32) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform2D::from_translation(at));
    world.insert(entity, Collider2D::Circle { radius });
    entity
}

#[test]
fn a_ray_reports_where_it_met_the_surface() {
    let mut world = World::new();
    let wall = box_at(&mut world, Vec2::new(5.0, 0.0), Vec2::new(1.0, 1.0));

    let hit = ray(
        &world,
        Vec2::ZERO,
        Vec2::new(1.0, 0.0),
        10.0,
        QueryFilter::new(),
    )
    .expect("it is straight ahead");

    assert_eq!(hit.entity, wall);
    assert!((hit.distance - 4.0).abs() < NEAR, "{}", hit.distance);
    assert!(hit.point.distance(Vec2::new(4.0, 0.0)) < NEAR);
    assert!(
        hit.normal.distance(Vec2::new(-1.0, 0.0)) < NEAR,
        "{:?}",
        hit.normal
    );
}

#[test]
fn a_ray_stops_at_the_nearer_of_two() {
    let mut world = World::new();
    let near = box_at(&mut world, Vec2::new(3.0, 0.0), Vec2::splat(0.5));
    box_at(&mut world, Vec2::new(6.0, 0.0), Vec2::splat(0.5));

    let hit = ray(
        &world,
        Vec2::ZERO,
        Vec2::new(1.0, 0.0),
        10.0,
        QueryFilter::new(),
    )
    .expect("both are ahead");

    assert_eq!(hit.entity, near);
}

#[test]
fn a_ray_that_falls_short_finds_nothing() {
    let mut world = World::new();
    box_at(&mut world, Vec2::new(5.0, 0.0), Vec2::splat(0.5));

    assert!(ray(
        &world,
        Vec2::ZERO,
        Vec2::new(1.0, 0.0),
        2.0,
        QueryFilter::new()
    )
    .is_none());
}

#[test]
fn a_ray_hits_the_turned_face_of_a_turned_box() {
    // A box turned 45° reaches half a diagonal closer than its own extent.
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(
        entity,
        Transform2D::from_translation(Vec2::new(4.0, 0.0))
            .with_rotation(std::f32::consts::FRAC_PI_4),
    );
    world.insert(
        entity,
        Collider2D::Box {
            half_extents: Vec2::splat(1.0),
        },
    );

    let hit = ray(
        &world,
        Vec2::ZERO,
        Vec2::new(1.0, 0.0),
        10.0,
        QueryFilter::new(),
    )
    .expect("straight ahead");

    let expected = 4.0 - std::f32::consts::SQRT_2;
    assert!(
        (hit.distance - expected).abs() < NEAR,
        "{} vs {expected}",
        hit.distance
    );
    assert!(
        hit.normal.dot(Vec2::new(-1.0, 0.0)) > 0.0,
        "{:?}",
        hit.normal
    );
}

#[test]
fn a_ray_that_starts_inside_hits_at_no_distance() {
    let mut world = World::new();
    let entity = box_at(&mut world, Vec2::ZERO, Vec2::splat(1.0));

    let hit = ray(
        &world,
        Vec2::ZERO,
        Vec2::new(1.0, 0.0),
        10.0,
        QueryFilter::new(),
    )
    .expect("it is inside it");

    assert_eq!(hit.entity, entity);
    assert_eq!(hit.distance, 0.0);
    assert!(
        hit.normal.distance(Vec2::new(-1.0, 0.0)) < NEAR,
        "the way back"
    );
}

#[test]
fn a_ray_with_no_direction_or_no_length_finds_nothing() {
    let mut world = World::new();
    box_at(&mut world, Vec2::ZERO, Vec2::splat(1.0));

    assert!(ray(&world, Vec2::ZERO, Vec2::ZERO, 10.0, QueryFilter::new()).is_none());
    assert!(ray(
        &world,
        Vec2::ZERO,
        Vec2::new(1.0, 0.0),
        0.0,
        QueryFilter::new()
    )
    .is_none());
    assert!(ray(
        &world,
        Vec2::ZERO,
        Vec2::new(1.0, 0.0),
        -1.0,
        QueryFilter::new()
    )
    .is_none());
}

#[test]
fn a_ray_meets_a_circle_at_its_edge() {
    let mut world = World::new();
    circle_at(&mut world, Vec2::new(0.0, 5.0), 2.0);

    let hit = ray(
        &world,
        Vec2::ZERO,
        Vec2::new(0.0, 1.0),
        10.0,
        QueryFilter::new(),
    )
    .expect("above");

    assert!((hit.distance - 3.0).abs() < NEAR, "{}", hit.distance);
    assert!(hit.normal.distance(Vec2::new(0.0, -1.0)) < NEAR);
}

#[test]
fn a_ray_pointing_away_from_a_circle_finds_nothing() {
    let mut world = World::new();
    circle_at(&mut world, Vec2::new(0.0, 5.0), 2.0);

    assert!(ray(
        &world,
        Vec2::ZERO,
        Vec2::new(0.0, -1.0),
        10.0,
        QueryFilter::new()
    )
    .is_none());
}

#[test]
fn a_query_ignores_what_it_excludes() {
    let mut world = World::new();
    let caster = box_at(&mut world, Vec2::ZERO, Vec2::splat(1.0));
    let wall = box_at(&mut world, Vec2::new(5.0, 0.0), Vec2::splat(1.0));

    let hit = ray(
        &world,
        Vec2::ZERO,
        Vec2::new(1.0, 0.0),
        10.0,
        QueryFilter::new().excluding(caster),
    )
    .expect("the wall is still there");

    assert_eq!(hit.entity, wall);
}

#[test]
fn a_query_only_finds_the_layers_it_looks_at() {
    let mut world = World::new();
    let ground = box_at(&mut world, Vec2::new(0.0, -5.0), Vec2::new(10.0, 1.0));
    world.insert(ground, CollisionLayers::on(1));
    let water = box_at(&mut world, Vec2::new(0.0, -2.0), Vec2::new(10.0, 1.0));
    world.insert(water, CollisionLayers::on(2));

    let hit = ray(
        &world,
        Vec2::ZERO,
        Vec2::new(0.0, -1.0),
        10.0,
        QueryFilter::new().looking_at(CollisionLayers::bit(1)),
    )
    .expect("the ground is on layer one");

    assert_eq!(hit.entity, ground, "the water is nearer and not looked at");
}

#[test]
fn a_query_walks_past_sensors_unless_asked() {
    let mut world = World::new();
    let trigger = box_at(&mut world, Vec2::new(2.0, 0.0), Vec2::splat(1.0));
    world.insert(trigger, Sensor);
    let wall = box_at(&mut world, Vec2::new(6.0, 0.0), Vec2::splat(1.0));

    let solid = ray(
        &world,
        Vec2::ZERO,
        Vec2::new(1.0, 0.0),
        10.0,
        QueryFilter::new(),
    )
    .expect("the wall");
    assert_eq!(solid.entity, wall);

    let either = ray(
        &world,
        Vec2::ZERO,
        Vec2::new(1.0, 0.0),
        10.0,
        QueryFilter::new().with_sensors(),
    )
    .expect("the trigger");
    assert_eq!(either.entity, trigger);
}

#[test]
fn a_degenerate_shape_is_not_a_surface() {
    let mut world = World::new();
    circle_at(&mut world, Vec2::new(3.0, 0.0), 0.0);

    assert!(ray(
        &world,
        Vec2::ZERO,
        Vec2::new(1.0, 0.0),
        10.0,
        QueryFilter::new()
    )
    .is_none());
}

#[test]
fn a_point_finds_what_covers_it() {
    let mut world = World::new();
    let entity = box_at(&mut world, Vec2::ZERO, Vec2::splat(1.0));

    assert_eq!(
        point(&world, Vec2::new(0.5, -0.5), QueryFilter::new()),
        Some(entity)
    );
    assert_eq!(point(&world, Vec2::new(2.0, 0.0), QueryFilter::new()), None);
}

#[test]
fn a_point_respects_a_turned_box() {
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(
        entity,
        Transform2D::default().with_rotation(std::f32::consts::FRAC_PI_4),
    );
    world.insert(
        entity,
        Collider2D::Box {
            half_extents: Vec2::splat(1.0),
        },
    );

    // Inside the bounds of the turned box, outside the box itself.
    assert_eq!(point(&world, Vec2::new(0.9, 0.9), QueryFilter::new()), None);
    assert_eq!(
        point(&world, Vec2::new(0.9, 0.0), QueryFilter::new()),
        Some(entity)
    );
}

#[test]
fn a_point_finds_a_circle_by_its_radius() {
    let mut world = World::new();
    let entity = circle_at(&mut world, Vec2::ZERO, 1.0);

    assert_eq!(
        point(&world, Vec2::new(0.9, 0.0), QueryFilter::new()),
        Some(entity)
    );
    assert_eq!(point(&world, Vec2::new(0.8, 0.8), QueryFilter::new()), None);
}

#[test]
fn an_overlap_reports_every_shape_in_the_box() {
    let mut world = World::new();
    let a = box_at(&mut world, Vec2::ZERO, Vec2::splat(0.5));
    let b = circle_at(&mut world, Vec2::new(1.0, 0.0), 0.5);
    box_at(&mut world, Vec2::new(20.0, 0.0), Vec2::splat(0.5));

    let found = overlap_aabb(
        &world,
        Vec2::new(-2.0, -2.0),
        Vec2::new(2.0, 2.0),
        QueryFilter::new(),
    );

    assert_eq!(found, vec![a, b]);
}

#[test]
fn an_overlap_tests_the_shape_and_not_its_bounds() {
    // The corner of a turned box: its bounds cover the query, its body
    // does not.
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(
        entity,
        Transform2D::default().with_rotation(std::f32::consts::FRAC_PI_4),
    );
    world.insert(
        entity,
        Collider2D::Box {
            half_extents: Vec2::splat(1.0),
        },
    );

    let found = overlap_aabb(
        &world,
        Vec2::new(1.2, 1.2),
        Vec2::new(1.35, 1.35),
        QueryFilter::new(),
    );

    assert!(found.is_empty(), "the corner is empty space");
}

#[test]
fn an_empty_or_inverted_box_overlaps_nothing() {
    let mut world = World::new();
    box_at(&mut world, Vec2::ZERO, Vec2::splat(1.0));

    assert!(overlap_aabb(&world, Vec2::ZERO, Vec2::ZERO, QueryFilter::new()).is_empty());
    assert!(overlap_aabb(
        &world,
        Vec2::splat(1.0),
        Vec2::splat(-1.0),
        QueryFilter::new()
    )
    .is_empty());
}
