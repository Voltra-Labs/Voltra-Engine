//! Capsules and convex polygons through everything boxes and circles go
//! through: queries for every pair of shapes, stacks that settle, a capsule
//! character on a polygon slope, sensors and one-way platforms.

mod common;

use common::*;
use voltra_ecs::{Entity, World};
use voltra_physics::query::{self, QueryFilter};
use voltra_physics::{move_and_slide, ImpulseCache, PhysicsWorld};
use voltra_render::glam::Vec2;
use voltra_scene::{
    CharacterBody2D, Collider, ConvexPolygon, OneWay, RigidBody, Sensor, Transform,
};

fn polygon(points: &[(f32, f32)], radius: f32) -> Collider {
    let points: Vec<Vec2> = points.iter().map(|(x, y)| Vec2::new(*x, *y)).collect();
    Collider::Polygon {
        points: ConvexPolygon::new(&points).expect("a polygon"),
        radius,
    }
}

/// A regular hexagon with flat top and bottom, `r` from centre to corner.
fn hexagon(r: f32) -> Collider {
    let points: Vec<Vec2> = (0..6)
        .map(|i| Vec2::from_angle(i as f32 * std::f32::consts::FRAC_PI_3) * r)
        .collect();
    Collider::Polygon {
        points: ConvexPolygon::new(&points).expect("a hexagon"),
        radius: 0.0,
    }
}

fn capsule(half_height: f32, radius: f32) -> Collider {
    Collider::Capsule {
        half_height,
        radius,
    }
}

/// One of each shape, each reaching 0.5 either side of its centre along x.
fn every_shape() -> [(&'static str, Collider); 5] {
    [
        (
            "box",
            Collider::Box {
                half_extents: Vec2::new(0.5, 0.5),
            },
        ),
        ("circle", Collider::Circle { radius: 0.5 }),
        ("capsule", capsule(1.0, 0.5)),
        (
            "polygon",
            polygon(
                &[
                    (-0.5, -0.5),
                    (0.5, -0.5),
                    (0.5, 0.2),
                    (0.0, 0.6),
                    (-0.5, 0.2),
                ],
                0.0,
            ),
        ),
        (
            "rounded polygon",
            Collider::Polygon {
                points: ConvexPolygon::rectangle(Vec2::splat(0.3)).expect("a square"),
                radius: 0.2,
            },
        ),
    ]
}

fn spawn(world: &mut World, collider: Collider, at: Transform) -> Entity {
    let entity = world.spawn();
    world.insert(entity, at);
    world.insert(entity, collider);
    entity
}

#[test]
fn every_query_answers_for_every_shape() {
    for (name, shape) in every_shape() {
        let mut world = World::new();
        let target = spawn(&mut world, shape, Transform::default());
        let any = QueryFilter::default();

        assert_eq!(
            query::point(&world, Vec2::new(0.1, 0.1), any),
            Some(target),
            "{name}"
        );
        assert_eq!(
            query::point(&world, Vec2::new(0.7, 0.0), any),
            None,
            "{name}"
        );

        let hit = query::ray(&world, Vec2::new(-5.0, 0.0), Vec2::X, 10.0, any)
            .unwrap_or_else(|| panic!("{name}: the ray misses"));
        assert!((hit.distance - 4.5).abs() < 1e-4, "{name}: {hit:?}");
        assert!(
            (hit.normal - Vec2::NEG_X).length() < 1e-4,
            "{name}: {hit:?}"
        );
        assert!(query::ray(&world, Vec2::new(-5.0, 0.0), Vec2::X, 4.4, any).is_none());

        let inside = query::overlap_aabb(&world, Vec2::new(0.3, -0.1), Vec2::new(0.4, 0.1), any);
        assert_eq!(inside, vec![target], "{name}");
        let beside = query::overlap_aabb(&world, Vec2::new(0.6, -0.1), Vec2::new(0.8, 0.1), any);
        assert!(beside.is_empty(), "{name}: {beside:?}");
    }
}

#[test]
fn every_shape_is_cast_against_every_shape() {
    for (moved_name, moved) in every_shape() {
        for (still_name, still) in every_shape() {
            let mut world = World::new();
            let target = spawn(&mut world, still, Transform::default());
            let from = Transform::from_translation(Vec2::new(-5.0, 0.0));
            let hit = query::cast(
                &world,
                &moved,
                &from,
                Vec2::new(10.0, 0.0),
                QueryFilter::default(),
            )
            .unwrap_or_else(|| panic!("{moved_name} misses {still_name}"));

            assert_eq!(hit.entity, target);
            assert!(
                (hit.distance - 4.0).abs() < 1e-3,
                "{moved_name} at {still_name}: {hit:?}"
            );
            assert!(
                (hit.normal - Vec2::NEG_X).length() < 1e-3,
                "{moved_name} at {still_name}: {hit:?}"
            );
            assert_eq!(hit.depth, 0.0);
        }
    }
}

#[test]
fn a_capsule_cast_onto_a_turned_polygon_meets_its_face() {
    let mut world = World::new();
    let slope = 30.0f32.to_radians();
    spawn(
        &mut world,
        polygon(&[(-5.0, -0.5), (5.0, -0.5), (5.0, 0.5), (-5.0, 0.5)], 0.0),
        Transform::default().with_rotation(slope),
    );
    let from = Transform::from_translation(Vec2::new(0.0, 3.0));
    let hit = query::cast(
        &world,
        &capsule(0.5, 0.25),
        &from,
        Vec2::new(0.0, -5.0),
        QueryFilter::default(),
    )
    .expect("it lands");

    let up = Vec2::from_angle(slope).perp();
    assert!((hit.normal - up).length() < 1e-3, "{hit:?}");
    // The cap's bottom meets the face: the centre is radius off it, which
    // straight down is 0.5 + 0.25/cos.
    let surface = 0.5 / slope.cos();
    let expected = 3.0 - surface - 0.25 / slope.cos() - 0.25;
    assert!(
        (hit.distance - expected).abs() < 2e-3,
        "{hit:?} vs {expected}"
    );
}

#[test]
fn a_cast_from_inside_reports_the_way_out_and_a_slide_along_a_touch_hits_nothing() {
    let mut world = World::new();
    let floor = spawn(
        &mut world,
        polygon(&[(-5.0, -1.0), (5.0, -1.0), (5.0, 0.0), (-5.0, 0.0)], 0.0),
        Transform::default(),
    );
    let shape = capsule(0.5, 0.25);

    let sunk = Transform::from_translation(Vec2::new(0.0, 0.4));
    let hit =
        query::cast(&world, &shape, &sunk, Vec2::X, QueryFilter::default()).expect("overlapping");
    assert_eq!((hit.entity, hit.fraction), (floor, 0.0));
    assert!((hit.depth - 0.1).abs() < 1e-4, "{hit:?}");
    assert!((hit.normal - Vec2::Y).length() < 1e-4, "{hit:?}");

    // Resting a hair above the floor, sliding along it and lifting off it.
    let resting = Transform::from_translation(Vec2::new(0.0, 0.5 + 2e-6));
    for motion in [Vec2::X, Vec2::new(1.0, 0.2)] {
        let hit = query::cast(&world, &shape, &resting, motion, QueryFilter::default());
        assert!(hit.is_none(), "{motion}: {hit:?}");
    }
    assert!(
        query::cast(
            &world,
            &shape,
            &resting,
            Vec2::NEG_Y,
            QueryFilter::default()
        )
        .is_some(),
        "but pressing into it does"
    );
}

#[test]
fn a_ray_meets_a_capsules_cap_and_misses_past_it() {
    let mut world = World::new();
    spawn(&mut world, capsule(1.0, 0.5), Transform::default());
    let any = QueryFilter::default();

    let down = query::ray(&world, Vec2::new(0.0, 5.0), Vec2::NEG_Y, 10.0, any).expect("the cap");
    assert!((down.distance - 4.0).abs() < 1e-5, "{down:?}");
    assert!((down.normal - Vec2::Y).length() < 1e-5, "{down:?}");

    let slanted =
        query::ray(&world, Vec2::new(0.3, 5.0), Vec2::NEG_Y, 10.0, any).expect("off-centre");
    let expected = 5.0 - (0.5 + (0.25f32 - 0.09).sqrt());
    assert!((slanted.distance - expected).abs() < 1e-4, "{slanted:?}");
    assert!(slanted.normal.x > 0.0, "{slanted:?}");

    assert!(query::ray(&world, Vec2::new(0.51, 5.0), Vec2::NEG_Y, 10.0, any).is_none());
}

/// A static polygon floor whose top is at y = 0.
fn polygon_floor(world: &mut World) -> Entity {
    spawn(
        world,
        polygon(
            &[(-20.0, -1.0), (20.0, -1.0), (20.0, 0.0), (-20.0, 0.0)],
            0.0,
        ),
        Transform::default(),
    )
}

fn dynamic(world: &mut World, collider: Collider, at: Transform) -> Entity {
    let entity = spawn(world, collider, at);
    world.insert(entity, RigidBody::new_dynamic(1.0));
    entity
}

#[test]
fn a_stack_of_hexagons_settles_and_stays_upright() {
    let mut world = World::new();
    polygon_floor(&mut world);
    let height = 2.0 * 0.5 * 60f32.to_radians().sin();
    let stack: Vec<Entity> = (0..5)
        .map(|k| {
            dynamic(
                &mut world,
                hexagon(0.5),
                Transform::from_translation(Vec2::new(
                    0.0,
                    height * 0.5 + k as f32 * height * 1.02,
                )),
            )
        })
        .collect();

    // Through the world, which puts a settled island to sleep: that is what
    // settling ends in.
    let mut physics = PhysicsWorld::new();
    steps(&mut world, &mut physics, 300);
    let settled: Vec<Vec2> = stack.iter().map(|e| translation(&world, *e)).collect();
    steps(&mut world, &mut physics, 60);
    assert_eq!(physics.asleep_count(), stack.len(), "the stack fell asleep");

    for (k, entity) in stack.iter().enumerate() {
        let at = translation(&world, *entity);
        let expected = height * 0.5 + k as f32 * height;
        assert!(
            (at.y - expected).abs() < 0.03,
            "hexagon {k} at {at}, wanted y {expected}"
        );
        // A box stack drifts as far, sideways, under this solver.
        assert!(at.x.abs() < 0.05, "hexagon {k} slid to {at}");
        assert!(rotation(&world, *entity).abs() < 0.02, "hexagon {k} turned");
        let drift = (at - settled[k]).length();
        assert!(drift < 0.005, "hexagon {k} still moving: {drift}");
    }
}

#[test]
fn capsules_dropped_lying_settle_on_each_other() {
    let mut world = World::new();
    polygon_floor(&mut world);
    let lying = |y: f32| {
        Transform::from_translation(Vec2::new(0.0, y)).with_rotation(std::f32::consts::FRAC_PI_2)
    };
    let low = dynamic(&mut world, capsule(1.0, 0.25), lying(0.3));
    let high = dynamic(&mut world, capsule(1.0, 0.25), lying(0.9));

    let mut physics = PhysicsWorld::new();
    steps(&mut world, &mut physics, 300);
    let settled = [translation(&world, low), translation(&world, high)];
    steps(&mut world, &mut physics, 60);
    assert_eq!(physics.asleep_count(), 2, "both fell asleep");

    assert!(
        (translation(&world, low).y - 0.25).abs() < 0.02,
        "{}",
        translation(&world, low)
    );
    assert!(
        (translation(&world, high).y - 0.75).abs() < 0.03,
        "{}",
        translation(&world, high)
    );
    for (entity, before) in [low, high].into_iter().zip(settled) {
        let drift = (translation(&world, entity) - before).length();
        assert!(drift < 0.005, "still moving: {drift}");
        assert!((rotation(&world, entity) - std::f32::consts::FRAC_PI_2).abs() < 0.05);
    }
}

#[test]
fn a_hexagon_rolls_down_a_steep_slope() {
    // Steeper than a hexagon can rest on: it turns over its corners, as a
    // box on a corner does — the polygon's inertia is a real one.
    let mut world = World::new();
    spawn(
        &mut world,
        polygon(
            &[(-10.0, -1.0), (10.0, -1.0), (10.0, 0.0), (-10.0, 0.0)],
            0.0,
        ),
        Transform::default().with_rotation(-0.6),
    );
    let hexagon_body = dynamic(
        &mut world,
        hexagon(0.4),
        Transform::from_translation(Vec2::new(0.0, 0.6)),
    );
    let mut cache = ImpulseCache::default();
    run(&mut world, &mut cache, G, 120);

    assert!(
        translation(&world, hexagon_body).x > 1.0,
        "{}",
        translation(&world, hexagon_body)
    );
    assert!(
        rotation(&world, hexagon_body) < -0.5,
        "it rolled: {}",
        rotation(&world, hexagon_body)
    );
}

fn steps(world: &mut World, physics: &mut PhysicsWorld, count: usize) {
    for _ in 0..count {
        physics.step_once(world, G);
    }
}

fn character(world: &mut World, collider: Collider, at: Vec2) -> Entity {
    let entity = spawn(world, collider, Transform::from_translation(at));
    world.insert(entity, CharacterBody2D::default());
    entity
}

fn walk(world: &mut World, me: Entity, speed: f32, ticks: u32) -> Vec2 {
    let mut velocity = Vec2::ZERO;
    for _ in 0..ticks {
        velocity.y -= 9.81 * DT;
        velocity.x = speed;
        velocity = move_and_slide(world, me, velocity, DT)
            .expect("a character")
            .velocity;
    }
    velocity
}

#[test]
fn a_capsule_character_walks_up_a_polygon_slope() {
    let mut world = World::new();
    polygon_floor(&mut world);
    // A ramp rising 25° from x = 1 to x = 7, its top flat beyond.
    let rise = 6.0 * 25f32.to_radians().tan();
    spawn(
        &mut world,
        polygon(&[(1.0, 0.0), (7.0, 0.0), (7.0, rise)], 0.0),
        Transform::default(),
    );
    spawn(
        &mut world,
        polygon(&[(7.0, 0.0), (15.0, 0.0), (15.0, rise), (7.0, rise)], 0.0),
        Transform::default(),
    );
    let me = character(&mut world, capsule(0.5, 0.25), Vec2::new(-1.0, 0.6));
    world.get_mut::<CharacterBody2D>(me).expect("c").step_height = 0.0;

    walk(&mut world, me, 2.0, 30);
    let start = translation(&world, me);
    assert!(
        (start.y - (0.5 + CharacterBody2D::default().skin)).abs() < 1e-3,
        "{start}"
    );

    let mut highest = start.y;
    for _ in 0..420 {
        walk(&mut world, me, 2.0, 1);
        let at = translation(&world, me);
        // Never into the ramp: the capsule's bottom stays on or above it.
        let ground = if at.x < 1.0 {
            0.0
        } else {
            ((at.x - 1.0) * 25f32.to_radians().tan()).min(rise)
        };
        assert!(at.y - 0.5 >= ground - 0.26, "sunk at {at}");
        highest = highest.max(at.y);
    }
    let end = translation(&world, me);
    assert!(end.x > 8.0, "it got over the top: {end}");
    assert!(
        (end.y - (rise + 0.5)).abs() < 0.05,
        "standing on the top: {end}"
    );
    assert!(world.get::<CharacterBody2D>(me).expect("c").is_on_floor());
    assert!(highest < rise + 0.6, "it did not bounce: {highest}");
}

#[test]
fn a_capsule_character_crosses_a_floor_of_tiles_without_catching() {
    // The reason for a capsule: its rounded bottom does not meet the next
    // tile's corner as a wall, even with stepping turned off.
    let mut world = World::new();
    for i in 0..20 {
        spawn_box(
            &mut world,
            Vec2::new(i as f32, -0.5),
            Vec2::splat(0.5),
            None,
        );
    }
    let me = character(&mut world, capsule(0.5, 0.25), Vec2::new(0.0, 0.6));
    world.get_mut::<CharacterBody2D>(me).expect("c").step_height = 0.0;

    walk(&mut world, me, 3.0, 30);
    let start = translation(&world, me).x;
    walk(&mut world, me, 3.0, 120);
    let travelled = translation(&world, me).x - start;

    assert!((travelled - 6.0).abs() < 0.05, "travelled {travelled}");
}

#[test]
fn a_polygon_sensor_reports_a_capsule_and_stops_nothing() {
    let mut world = World::new();
    polygon_floor(&mut world);
    let zone = spawn(
        &mut world,
        hexagon(1.0),
        Transform::from_translation(Vec2::new(0.0, 2.0)),
    );
    world.insert(zone, Sensor);
    let falling = dynamic(
        &mut world,
        capsule(0.5, 0.25),
        Transform::from_translation(Vec2::new(0.0, 5.0)),
    );

    let mut physics = PhysicsWorld::new();
    let mut entered = false;
    for _ in 0..180 {
        physics.step_once(&mut world, G);
        entered |= physics.touching().contains(zone, falling);
    }
    assert!(entered, "the sensor saw it pass");
    let y = translation(&world, falling).y;
    assert!(
        (y - 0.5).abs() < 0.03,
        "fell through the sensor onto the floor: {y}"
    );
}

#[test]
fn a_capsule_lands_on_a_one_way_polygon_and_rises_through_it() {
    let platform_shape = polygon(&[(-2.0, -0.2), (2.0, -0.2), (2.0, 0.0), (-2.0, 0.0)], 0.0);

    let mut world = World::new();
    let platform = spawn(&mut world, platform_shape, Transform::default());
    world.insert(platform, OneWay::default());
    let dropped = dynamic(
        &mut world,
        capsule(0.5, 0.25),
        Transform::from_translation(Vec2::new(0.0, 1.5)),
    );
    let mut physics = PhysicsWorld::new();
    for _ in 0..180 {
        physics.step_once(&mut world, G);
    }
    let y = translation(&world, dropped).y;
    assert!((y - 0.5).abs() < 0.03, "resting on top: {y}");

    let mut world = World::new();
    let platform = spawn(&mut world, platform_shape, Transform::default());
    world.insert(platform, OneWay::default());
    let rising = dynamic(
        &mut world,
        capsule(0.5, 0.25),
        Transform::from_translation(Vec2::new(0.0, -1.0)),
    );
    world.get_mut::<RigidBody>(rising).expect("a body").velocity = Vec2::new(0.0, 9.0);
    let mut physics = PhysicsWorld::new();
    let mut highest = f32::MIN;
    for _ in 0..240 {
        physics.step_once(&mut world, G);
        highest = highest.max(translation(&world, rising).y);
    }
    assert!(highest > 1.5, "it went up through: {highest}");
    let y = translation(&world, rising).y;
    assert!((y - 0.5).abs() < 0.03, "and came down on top: {y}");
}
