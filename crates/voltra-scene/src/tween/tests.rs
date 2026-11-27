use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::*;
use crate::{Sprite2D, Transform2D};

fn sprite(world: &mut World) -> Entity {
    let e = world.spawn();
    world.insert(e, Transform2D::default());
    world.insert(e, Sprite2D::new([1.0, 1.0, 1.0, 1.0]));
    e
}

fn position(world: &World, e: Entity) -> Vec2 {
    world.get::<Transform2D>(e).expect("placed").translation
}

fn alpha(world: &World, e: Entity) -> f32 {
    world.get::<Sprite2D>(e).expect("a sprite").color[3]
}

#[test]
fn a_tween_moves_its_property_and_lands_on_the_value() {
    let mut world = World::new();
    let e = sprite(&mut world);
    let mut tweens = Tweens::default();
    let fade = tweens.tween(
        e,
        Property::SpriteColor,
        Value::Color([1.0, 1.0, 1.0, 0.0]),
        1.0,
        Ease::Linear,
    );
    tweens.step(&mut world, 0.25);
    assert!((alpha(&world, e) - 0.75).abs() < 1e-5);
    assert!(tweens.is_running(fade) && !tweens.finished(fade));
    tweens.step(&mut world, 1.0);
    assert_eq!(alpha(&world, e), 0.0, "exactly the value at the end");
    assert!(tweens.finished(fade) && !tweens.is_running(fade));
    tweens.step(&mut world, 0.1);
    assert!(!tweens.finished(fade), "finished in one step only");
}

#[test]
fn an_ease_shapes_it() {
    let mut world = World::new();
    let e = sprite(&mut world);
    let mut tweens = Tweens::default();
    tweens.tween(
        e,
        Property::Translation2D,
        Value::Vec2(Vec2::new(10.0, 0.0)),
        1.0,
        Ease::EaseIn,
    );
    tweens.step(&mut world, 0.5);
    assert!(
        (position(&world, e).x - 2.5).abs() < 1e-4,
        "a quarter of the way at half time"
    );
}

#[test]
fn a_chain_starts_where_the_last_left_off_with_the_rest_of_the_frame() {
    let mut world = World::new();
    let e = sprite(&mut world);
    let mut tweens = Tweens::default();
    let right = tweens.tween(
        e,
        Property::Translation2D,
        Value::Vec2(Vec2::new(4.0, 0.0)),
        1.0,
        Ease::Linear,
    );
    let up = tweens.then(
        right,
        e,
        Property::Translation2D,
        Value::Vec2(Vec2::new(4.0, 4.0)),
        1.0,
        Ease::Linear,
    );
    tweens.step(&mut world, 0.5);
    assert_eq!(position(&world, e), Vec2::new(2.0, 0.0));
    tweens.step(&mut world, 0.75);
    assert!(tweens.finished(right));
    let p = position(&world, e);
    assert!(
        (p - Vec2::new(4.0, 1.0)).length() < 1e-4,
        "a quarter up: {p}"
    );
    tweens.step(&mut world, 1.0);
    assert!(tweens.finished(up));
    assert_eq!(position(&world, e), Vec2::new(4.0, 4.0));
    assert!(tweens.is_empty());
}

#[test]
fn a_despawned_entity_takes_its_tweens_and_their_chains_with_it() {
    let mut world = World::new();
    let e = sprite(&mut world);
    let other = sprite(&mut world);
    let mut tweens = Tweens::default();
    let first = tweens.tween(
        e,
        Property::Scale2D,
        Value::Vec2(Vec2::splat(2.0)),
        1.0,
        Ease::Linear,
    );
    let chained = tweens.then(
        first,
        other,
        Property::Scale2D,
        Value::Vec2(Vec2::splat(3.0)),
        1.0,
        Ease::Linear,
    );
    tweens.step(&mut world, 0.1);
    world.despawn(e);
    tweens.step(&mut world, 5.0);
    assert!(!tweens.is_running(first) && !tweens.is_running(chained));
    assert!(!tweens.finished(first), "stopped, not finished");
    assert_eq!(
        world.get::<Transform2D>(other).expect("placed").scale,
        Vec2::ONE
    );
}

#[test]
fn a_newer_tween_on_a_property_takes_it_over() {
    let mut world = World::new();
    let e = sprite(&mut world);
    let mut tweens = Tweens::default();
    let old = tweens.tween(
        e,
        Property::Translation2D,
        Value::Vec2(Vec2::new(10.0, 0.0)),
        1.0,
        Ease::Linear,
    );
    tweens.step(&mut world, 0.5);
    let new = tweens.tween(
        e,
        Property::Translation2D,
        Value::Vec2(Vec2::new(-10.0, 0.0)),
        1.0,
        Ease::Linear,
    );
    tweens.step(&mut world, 0.5);
    assert!(!tweens.is_running(old));
    assert_eq!(position(&world, e).x, -2.5, "from 5 halfway to -10");
    let _fade = tweens.tween(
        e,
        Property::SpriteColor,
        Value::Color([0.0; 4]),
        1.0,
        Ease::Linear,
    );
    tweens.step(&mut world, 0.1);
    assert!(tweens.is_running(new), "another property is left alone");
}

#[test]
fn cancelling_stops_it_where_it_is() {
    let mut world = World::new();
    let e = sprite(&mut world);
    let mut tweens = Tweens::default();
    let id = tweens.tween(
        e,
        Property::Rotation2D,
        Value::Float(1.0),
        1.0,
        Ease::Linear,
    );
    tweens.step(&mut world, 0.5);
    tweens.cancel(id);
    tweens.step(&mut world, 1.0);
    assert_eq!(world.get::<Transform2D>(e).expect("placed").rotation, 0.5);
}

#[test]
fn a_value_of_the_wrong_kind_is_refused_and_a_chain_behind_it_runs() {
    let mut world = World::new();
    let e = sprite(&mut world);
    let mut tweens = Tweens::default();
    let bad = tweens.tween(
        e,
        Property::Rotation2D,
        Value::Vec2(Vec2::ONE),
        1.0,
        Ease::Linear,
    );
    assert!(tweens.finished(bad) && !tweens.is_running(bad));
    let next = tweens.then(
        bad,
        e,
        Property::Rotation2D,
        Value::Float(2.0),
        0.0,
        Ease::Linear,
    );
    tweens.step(&mut world, 0.1);
    assert!(tweens.finished(next));
    assert_eq!(world.get::<Transform2D>(e).expect("placed").rotation, 2.0);
}

#[test]
fn a_tween_of_a_missing_component_is_dropped() {
    let mut world = World::new();
    let e = world.spawn();
    let mut tweens = Tweens::default();
    let id = tweens.tween(
        e,
        Property::PointLight2DRadius,
        Value::Float(3.0),
        1.0,
        Ease::Linear,
    );
    tweens.step(&mut world, 0.1);
    assert!(!tweens.is_running(id));
}
