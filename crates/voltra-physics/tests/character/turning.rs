//! A turning floor turning the character standing on it, when asked to.

use std::f32::consts::PI;

use super::*;

#[test]
fn a_turning_floor_turns_the_character_only_when_it_carries_rotation() {
    for carried in [true, false] {
        let mut world = World::new();
        let lift = block(&mut world, Vec2::ZERO, Vec2::new(2.0, 0.1), 0.0);
        let me = character(&mut world, Vec2::new(0.5, 0.5));
        if let Some(body) = world.get_mut::<CharacterBody2D>(me) {
            body.carry_rotation = carried;
        }
        let mut velocity = run(&mut world, me, Vec2::ZERO, None, 60);
        assert!(body(&world, me).is_on_floor());

        for _ in 0..10 {
            world.get_mut::<Transform2D>(lift).expect("lift").rotation += 0.03;
            velocity = run(&mut world, me, velocity, None, 1);
        }
        let turned = world.get::<Transform2D>(me).expect("placed").rotation;
        if carried {
            assert!((turned - 0.3).abs() < 1e-4, "{turned}");
        } else {
            assert_eq!(turned, 0.0);
        }
        assert!(body(&world, me).is_on_floor(), "still on it: {carried}");
        // Carried round the lift's centre either way.
        let expected = Vec2::from_angle(0.3).rotate(Vec2::new(0.5, 0.35));
        assert!(
            at(&world, me).distance(expected) < 0.1,
            "{carried}: {} against {expected}",
            at(&world, me)
        );

        // Off the floor, nothing turns it.
        world.despawn(lift);
        run(&mut world, me, velocity, None, 5);
        let after = world.get::<Transform2D>(me).expect("placed").rotation;
        assert_eq!(after, turned);
    }
}

#[test]
fn a_floor_turning_through_a_half_turn_turns_it_the_short_way() {
    let mut world = World::new();
    let lift = block(&mut world, Vec2::ZERO, Vec2::new(2.0, 0.1), 3.1);
    let me = character(&mut world, Vec2::new(0.0, 0.5));
    if let Some(body) = world.get_mut::<CharacterBody2D>(me) {
        body.carry_rotation = true;
    }
    let velocity = run(&mut world, me, Vec2::ZERO, None, 60);
    assert!(body(&world, me).is_on_floor());
    // From 3.1 on to -3.1: 0.083 further round, written the other way.
    world.get_mut::<Transform2D>(lift).expect("lift").rotation = -3.1;
    run(&mut world, me, velocity, None, 1);
    let turned = world.get::<Transform2D>(me).expect("placed").rotation;
    let expected = 2.0 * PI - 6.2;
    assert!((turned - expected).abs() < 1e-4, "{turned}");
}
