//! Boxes and circles cast at walls, floors, corners and each other.

use std::f32::consts::FRAC_PI_4;

use super::*;
use voltra_scene::Sensor;

const NEAR: f32 = 1e-4;

fn solid(world: &mut World, at: Vec2, collider: Collider2D) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform2D::from_translation(at));
    world.insert(entity, collider);
    entity
}

fn square(half: f32) -> Collider2D {
    Collider2D::Box {
        half_extents: Vec2::splat(half),
    }
}

fn ball(radius: f32) -> Collider2D {
    Collider2D::Circle { radius }
}

fn at(x: f32, y: f32) -> Transform2D {
    Transform2D::from_translation(Vec2::new(x, y))
}

fn near(a: Vec2, b: Vec2) -> bool {
    a.distance(b) < NEAR
}

#[test]
fn a_box_stops_at_a_wall_face_on() {
    let mut world = World::new();
    let wall = solid(&mut world, Vec2::new(5.0, 0.0), square(1.0));
    let hit = cast(
        &world,
        &square(0.5),
        &at(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        QueryFilter::new(),
    )
    .expect("the wall is in the way");
    assert_eq!(hit.entity, wall);
    assert!((hit.distance - 3.5).abs() < NEAR, "{}", hit.distance);
    assert!((hit.fraction - 0.35).abs() < NEAR);
    assert!(near(hit.normal, Vec2::new(-1.0, 0.0)), "{:?}", hit.normal);
    assert_eq!(hit.depth, 0.0);
}

#[test]
fn a_box_falls_onto_a_turned_floor_by_its_slope() {
    let mut world = World::new();
    let slope = world.spawn();
    world.insert(slope, Transform2D::default().with_rotation(FRAC_PI_4));
    world.insert(
        slope,
        Collider2D::Box {
            half_extents: Vec2::new(10.0, 0.5),
        },
    );
    let hit = cast(
        &world,
        &square(0.25),
        &at(0.0, 3.0),
        Vec2::new(0.0, -6.0),
        QueryFilter::new(),
    )
    .expect("it lands");
    let up_slope = Vec2::from_angle(FRAC_PI_4).perp();
    assert!(near(hit.normal, up_slope), "{:?}", hit.normal);
    // The box's lowest corner reaches the slope's face: the face is 0.5 off
    // the centre line along the normal, the corner reaches 0.25·√2 down it,
    // and a vertical drop covers that at cos 45° per unit.
    let along_normal = 0.5 + 0.25 * 2.0f32.sqrt();
    let landed = 3.0 - hit.distance;
    assert!(
        (landed - along_normal * 2.0f32.sqrt()).abs() < 1e-3,
        "{landed}"
    );
}

#[test]
fn sliding_along_a_floor_it_touches_does_not_hit_the_floor() {
    let mut world = World::new();
    solid(
        &mut world,
        Vec2::new(0.0, -1.0),
        Collider2D::Box {
            half_extents: Vec2::new(10.0, 0.5),
        },
    );
    let standing = at(0.0, 0.0);
    let shape = Collider2D::Box {
        half_extents: Vec2::splat(0.5),
    };
    assert!(cast(
        &world,
        &shape,
        &standing,
        Vec2::new(3.0, 0.0),
        QueryFilter::new()
    )
    .is_none());
    assert!(
        cast(
            &world,
            &shape,
            &standing,
            Vec2::new(0.0, 1.0),
            QueryFilter::new()
        )
        .is_none(),
        "leaving"
    );
    let down = cast(
        &world,
        &shape,
        &standing,
        Vec2::new(1.0, -1.0),
        QueryFilter::new(),
    )
    .expect("pressing into it");
    assert_eq!(down.fraction, 0.0);
    assert!(near(down.normal, Vec2::Y));
}

#[test]
fn a_shape_starting_inside_reports_the_way_out() {
    let mut world = World::new();
    solid(
        &mut world,
        Vec2::new(0.0, -1.0),
        Collider2D::Box {
            half_extents: Vec2::new(10.0, 0.5),
        },
    );
    let hit = cast(
        &world,
        &square(0.5),
        &at(0.0, -0.1),
        Vec2::new(1.0, 0.0),
        QueryFilter::new(),
    )
    .expect("it is in the floor");
    assert_eq!(hit.fraction, 0.0);
    assert!(near(hit.normal, Vec2::Y), "{:?}", hit.normal);
    assert!((hit.depth - 0.1).abs() < 1e-3, "{}", hit.depth);
    let still = cast(
        &world,
        &square(0.5),
        &at(0.0, -0.1),
        Vec2::ZERO,
        QueryFilter::new(),
    )
    .expect("no motion still finds the overlap");
    assert_eq!(still.depth, hit.depth);
}

#[test]
fn a_circle_meets_a_box_face_and_its_rounded_corner() {
    let mut world = World::new();
    solid(&mut world, Vec2::ZERO, square(1.0));
    let face = cast(
        &world,
        &ball(0.5),
        &at(-5.0, 0.0),
        Vec2::new(10.0, 0.0),
        QueryFilter::new(),
    )
    .expect("face on");
    assert!((face.distance - 3.5).abs() < NEAR);
    assert!(near(face.normal, Vec2::new(-1.0, 0.0)));

    // Diagonally at the corner: it meets the corner, not the grown box's
    // square corner, so it travels further than the faces would allow.
    let from = Vec2::splat(-3.0);
    let corner = cast(
        &world,
        &ball(0.5),
        &at(from.x, from.y),
        Vec2::splat(4.0),
        QueryFilter::new(),
    )
    .expect("at the corner");
    let expected = (from - Vec2::splat(-1.0)).length() - 0.5;
    assert!(
        (corner.distance - expected).abs() < 1e-3,
        "{} vs {expected}",
        corner.distance
    );
    assert!(
        near(corner.normal, Vec2::splat(-1.0).normalize()),
        "{:?}",
        corner.normal
    );

    // Through the grown box's square corner but past the corner's circle:
    // a miss.
    assert!(cast(
        &world,
        &ball(0.5),
        &at(-3.0, -0.1),
        Vec2::splat(3.0),
        QueryFilter::new()
    )
    .is_none());
}

#[test]
fn a_box_meets_a_circle_and_circles_meet_each_other() {
    let mut world = World::new();
    let post = solid(&mut world, Vec2::new(4.0, 0.0), ball(1.0));
    let boxed = cast(
        &world,
        &square(0.5),
        &at(0.0, 0.0),
        Vec2::new(8.0, 0.0),
        QueryFilter::new(),
    )
    .expect("box into ball");
    assert_eq!(boxed.entity, post);
    assert!((boxed.distance - 2.5).abs() < NEAR);
    assert!(near(boxed.normal, Vec2::new(-1.0, 0.0)));

    let rolled = cast(
        &world,
        &ball(0.5),
        &at(0.0, 0.0),
        Vec2::new(8.0, 0.0),
        QueryFilter::new(),
    )
    .expect("ball into ball");
    assert!((rolled.distance - 2.5).abs() < NEAR);
    assert!(near(rolled.normal, Vec2::new(-1.0, 0.0)));
}

#[test]
fn a_cast_takes_the_nearest_and_honours_its_filter() {
    let mut world = World::new();
    let me = solid(&mut world, Vec2::ZERO, square(0.5));
    let trigger = solid(&mut world, Vec2::new(2.0, 0.0), square(0.5));
    world.insert(trigger, Sensor);
    let near_wall = solid(&mut world, Vec2::new(4.0, 0.0), square(0.5));
    solid(&mut world, Vec2::new(6.0, 0.0), square(0.5));
    let hit = cast(
        &world,
        &square(0.5),
        &at(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        QueryFilter::new().excluding(me),
    )
    .expect("a wall");
    assert_eq!(hit.entity, near_wall, "past the sensor, and not itself");
}

#[test]
fn a_fast_cast_does_not_tunnel_and_a_short_one_falls_short() {
    let mut world = World::new();
    solid(
        &mut world,
        Vec2::new(50.0, 0.0),
        Collider2D::Box {
            half_extents: Vec2::new(0.01, 1.0),
        },
    );
    assert!(cast(
        &world,
        &square(0.1),
        &at(0.0, 0.0),
        Vec2::new(1000.0, 0.0),
        QueryFilter::new()
    )
    .is_some());
    assert!(cast(
        &world,
        &square(0.1),
        &at(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        QueryFilter::new()
    )
    .is_none());
    assert!(cast(
        &world,
        &square(0.1),
        &at(0.0, 0.0),
        Vec2::new(f32::NAN, 0.0),
        QueryFilter::new()
    )
    .is_none());
}

#[test]
fn a_one_way_collider_is_met_only_from_its_solid_side() {
    let mut world = World::new();
    let ledge = solid(
        &mut world,
        Vec2::ZERO,
        Collider2D::Box {
            half_extents: Vec2::new(2.0, 0.1),
        },
    );
    world.insert(ledge, voltra_scene::OneWay::default());
    let from_above = cast(
        &world,
        &square(0.2),
        &at(0.0, 1.0),
        Vec2::new(0.0, -2.0),
        QueryFilter::new(),
    );
    assert_eq!(from_above.map(|hit| hit.entity), Some(ledge));
    assert!(
        cast(
            &world,
            &square(0.2),
            &at(0.0, -1.0),
            Vec2::new(0.0, 2.0),
            QueryFilter::new()
        )
        .is_none(),
        "from below"
    );
    assert!(
        cast(
            &world,
            &square(0.2),
            &at(-3.0, 0.0),
            Vec2::new(6.0, 0.0),
            QueryFilter::new()
        )
        .is_none(),
        "from the side"
    );
    assert!(
        cast(
            &world,
            &square(0.2),
            &at(0.0, 0.1),
            Vec2::new(0.0, -1.0),
            QueryFilter::new()
        )
        .is_none(),
        "partly through"
    );
}
