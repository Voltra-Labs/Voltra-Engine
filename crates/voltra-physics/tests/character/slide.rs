//! Walking, landing, slopes, ceilings, corners, seams and steps.

use std::f32::consts::PI;

use super::*;

#[test]
fn it_lands_a_skin_above_the_floor_and_stops_falling() {
    let mut world = World::new();
    let ground = floor(&mut world);
    let me = character(&mut world, Vec2::new(0.0, 2.0));
    let velocity = run(&mut world, me, Vec2::ZERO, None, 120);
    let skin = CharacterBody2D::default().skin;
    assert!(
        (at(&world, me).y - (0.25 + skin)).abs() < 1e-3,
        "{}",
        at(&world, me).y
    );
    assert!(
        velocity.y.abs() < 1.0,
        "the fall is spent on landing: {velocity}"
    );
    assert!(body(&world, me).is_on_floor());
    assert_eq!(body(&world, me).contact.floor, Some(ground));
    assert_eq!(body(&world, me).contact.floor_normal, Vec2::Y);
}

#[test]
fn it_walks_along_the_floor_at_full_speed() {
    let mut world = World::new();
    floor(&mut world);
    let me = character(&mut world, Vec2::new(0.0, 0.3));
    run(&mut world, me, Vec2::ZERO, None, 30);
    let start = at(&world, me);
    run(&mut world, me, Vec2::ZERO, Some(3.0), 60);
    let walked = at(&world, me) - start;
    assert!((walked.x - 3.0).abs() < 1e-2, "{walked}");
    assert!(walked.y.abs() < 1e-3, "{walked}");
    assert!(body(&world, me).is_on_floor());
}

#[test]
fn a_wall_stops_it_and_takes_its_speed_into_the_wall() {
    let mut world = World::new();
    floor(&mut world);
    block(&mut world, Vec2::new(2.0, 1.0), Vec2::new(0.5, 1.0), 0.0);
    let me = character(&mut world, Vec2::new(0.0, 0.3));
    let velocity = run(&mut world, me, Vec2::ZERO, Some(4.0), 90);
    assert!(at(&world, me).x <= 1.5 - 0.25, "{}", at(&world, me).x);
    assert!(at(&world, me).x > 1.5 - 0.25 - 0.05, "{}", at(&world, me).x);
    assert!(body(&world, me).is_on_wall() && body(&world, me).is_on_floor());
    assert_eq!(body(&world, me).contact.wall_normal, Vec2::NEG_X);
    assert!(velocity.x.abs() < 1e-4, "{velocity}");
}

#[test]
fn a_gentle_slope_is_climbed_and_a_steep_one_is_a_wall() {
    for (degrees, climbs) in [(30.0f32, true), (60.0, false)] {
        let mut world = World::new();
        floor(&mut world);
        let angle = degrees.to_radians();
        // A long ramp rising to the right from x = 1.
        let half = Vec2::new(4.0, 0.05);
        let foot = Vec2::new(1.0, 0.0);
        let centre =
            foot + Vec2::from_angle(angle) * half.x + Vec2::from_angle(angle).perp() * -half.y;
        block(&mut world, centre, half, angle);
        let me = character(&mut world, Vec2::new(0.0, 0.3));
        run(&mut world, me, Vec2::ZERO, Some(2.0), 90);
        let height = at(&world, me).y;
        assert_eq!(height > 0.8, climbs, "{degrees}°: at {}", at(&world, me));
        if !climbs {
            assert!(body(&world, me).is_on_wall(), "{degrees}°");
        }
    }
}

#[test]
fn standing_on_a_slope_holds_unless_told_to_slide() {
    for stop in [true, false] {
        let mut world = World::new();
        block(
            &mut world,
            Vec2::ZERO,
            Vec2::new(10.0, 0.5),
            30.0f32.to_radians(),
        );
        let me = character(&mut world, Vec2::new(0.0, 1.5));
        world
            .get_mut::<CharacterBody2D>(me)
            .expect("c")
            .stop_on_slope = stop;
        run(&mut world, me, Vec2::ZERO, None, 60);
        let landed = at(&world, me);
        run(&mut world, me, Vec2::ZERO, None, 60);
        let drift = (at(&world, me) - landed).length();
        assert_eq!(drift < 1e-3, stop, "stop_on_slope {stop}: drifted {drift}");
        assert!(body(&world, me).is_on_floor());
    }
}

#[test]
fn a_jump_ends_on_the_ceiling_and_leaves_the_floor() {
    let mut world = World::new();
    floor(&mut world);
    block(&mut world, Vec2::new(0.0, 1.5), Vec2::new(5.0, 0.25), 0.0);
    let me = character(&mut world, Vec2::new(0.0, 0.3));
    run(&mut world, me, Vec2::ZERO, None, 10);
    let velocity = move_and_slide(&mut world, me, Vec2::new(0.0, 8.0), DT)
        .expect("c")
        .velocity;
    assert!(
        !body(&world, me).is_on_floor(),
        "a jump is not snapped back"
    );
    let mut velocity = velocity;
    let mut hit_ceiling = false;
    for _ in 0..30 {
        velocity.y += GRAVITY * DT;
        velocity = move_and_slide(&mut world, me, velocity, DT)
            .expect("c")
            .velocity;
        hit_ceiling |= body(&world, me).is_on_ceiling();
        assert!(
            at(&world, me).y <= 1.25 - 0.25 + 1e-4,
            "never through: {}",
            at(&world, me).y
        );
    }
    assert!(hit_ceiling);
}

#[test]
fn walking_down_a_slope_stays_on_it_and_off_a_ledge_falls() {
    let mut world = World::new();
    let angle = -20.0f32.to_radians();
    block(&mut world, Vec2::ZERO, Vec2::new(10.0, 0.5), angle);
    let me = character(&mut world, Vec2::new(-5.0, 3.0));
    run(&mut world, me, Vec2::ZERO, None, 120);
    for _ in 0..60 {
        run(&mut world, me, Vec2::ZERO, Some(5.0), 1);
        assert!(
            body(&world, me).is_on_floor(),
            "snapped down the slope at {}",
            at(&world, me)
        );
    }

    let mut world = World::new();
    block(&mut world, Vec2::new(-5.0, -0.5), Vec2::new(5.0, 0.5), 0.0);
    let me = character(&mut world, Vec2::new(-1.0, 0.3));
    run(&mut world, me, Vec2::ZERO, None, 30);
    run(&mut world, me, Vec2::ZERO, Some(5.0), 45);
    assert!(
        at(&world, me).y < -0.5,
        "off the edge and falling: {}",
        at(&world, me)
    );
    assert!(!body(&world, me).is_on_floor());
}

#[test]
fn it_is_pushed_out_of_what_it_starts_inside() {
    let mut world = World::new();
    floor(&mut world);
    let me = character(&mut world, Vec2::new(0.0, 0.1));
    move_and_slide(&mut world, me, Vec2::ZERO, DT).expect("c");
    assert!(at(&world, me).y >= 0.25, "{}", at(&world, me).y);
}

#[test]
fn a_v_shaped_corner_stops_it_without_sinking_in() {
    let mut world = World::new();
    block(
        &mut world,
        Vec2::new(-1.0, 0.0),
        Vec2::new(2.0, 0.2),
        PI / 4.0,
    );
    block(
        &mut world,
        Vec2::new(1.0, 0.0),
        Vec2::new(2.0, 0.2),
        -PI / 4.0,
    );
    let me = character(&mut world, Vec2::new(0.0, 2.5));
    let mut velocity = Vec2::ZERO;
    for _ in 0..240 {
        velocity.y += GRAVITY * DT;
        velocity = move_and_slide(&mut world, me, velocity, DT)
            .expect("c")
            .velocity;
    }
    let settled = at(&world, me);
    assert!(settled.x.abs() < 0.05, "{settled}");
    let manifold_free = voltra_physics::query::cast(
        &world,
        &Collider2D::Box {
            half_extents: Vec2::splat(0.25),
        },
        &Transform2D::from_translation(settled),
        Vec2::ZERO,
        voltra_physics::QueryFilter::new().excluding(me),
    );
    assert!(
        manifold_free.is_none_or(|hit| hit.depth < 1e-3),
        "{manifold_free:?}"
    );
}

#[test]
fn an_entity_that_is_not_a_character_is_not_moved() {
    let mut world = World::new();
    let plain = world.spawn();
    world.insert(plain, Transform2D::default());
    assert!(move_and_slide(&mut world, plain, Vec2::X, DT).is_none());
}

#[test]
fn a_floor_of_tiles_is_walked_over_without_catching_on_the_seams() {
    let mut world = World::new();
    for i in -5..20 {
        block(
            &mut world,
            Vec2::new(i as f32 + 0.5, -0.5),
            Vec2::splat(0.5),
            0.0,
        );
    }
    let me = character(&mut world, Vec2::new(0.0, 0.3));
    run(&mut world, me, Vec2::ZERO, None, 30);
    let start = at(&world, me);
    run(&mut world, me, Vec2::ZERO, Some(4.0), 120);
    let walked = at(&world, me) - start;
    assert!((walked.x - 8.0).abs() < 0.05, "{walked}");
    assert!(walked.y.abs() < 0.01, "{walked}");
    assert!(!body(&world, me).is_on_wall());
}

#[test]
fn a_low_ledge_is_stepped_onto_and_a_high_one_stops_it() {
    for (height, steps) in [(0.04f32, true), (0.2, false)] {
        let mut world = World::new();
        floor(&mut world);
        block(
            &mut world,
            Vec2::new(3.0, height / 2.0),
            Vec2::new(1.0, height / 2.0),
            0.0,
        );
        let me = character(&mut world, Vec2::new(0.0, 0.3));
        run(&mut world, me, Vec2::ZERO, Some(3.0), 60);
        let on_top = at(&world, me).x > 2.0;
        assert_eq!(on_top, steps, "{height}: at {}", at(&world, me));
        if !steps {
            assert!(body(&world, me).is_on_wall());
        }
    }
}
