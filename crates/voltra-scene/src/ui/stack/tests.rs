use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::*;
use crate::ui::UiLayout;
use crate::SceneId;

const SCREEN: Vec2 = Vec2::new(800.0, 600.0);

fn ui(world: &mut World, rect: UiRect) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, rect);
    entity
}

/// A 200 × 300 column at the top-left, 10 points of padding, 5 between.
fn column(world: &mut World, stack: UiStack) -> Entity {
    let panel = ui(
        world,
        UiRect::pinned(Vec2::ZERO, Vec2::ZERO, Vec2::new(200.0, 300.0)),
    );
    world.insert(panel, stack);
    panel
}

fn stack() -> UiStack {
    UiStack {
        spacing: 5.0,
        padding: Edges::uniform(10.0),
        ..UiStack::default()
    }
}

fn child(world: &mut World, panel: Entity, size: Vec2, slot: i32) -> Entity {
    let e = ui(
        world,
        UiRect::pinned(Vec2::ZERO, Vec2::splat(999.0), size).with_slot(slot),
    );
    hierarchy::set_parent(world, e, panel).expect("parents");
    e
}

#[test]
fn a_column_places_its_children_one_under_another() {
    let mut world = World::new();
    let panel = column(&mut world, stack());
    let a = child(&mut world, panel, Vec2::new(50.0, 20.0), 0);
    let b = child(&mut world, panel, Vec2::new(50.0, 30.0), 1);

    let layout = UiLayout::from_world(&world, SCREEN);
    // Stretched across the padded box; the offset of 999 is overridden.
    assert_eq!(
        layout.rect(a),
        Some(Rect::new(Vec2::splat(10.0), Vec2::new(190.0, 30.0)))
    );
    assert_eq!(
        layout.rect(b),
        Some(Rect::new(Vec2::new(10.0, 35.0), Vec2::new(190.0, 65.0)))
    );
    assert_eq!(layout.depth(b), 1);
}

#[test]
fn slots_decide_the_order_not_the_spawn() {
    let mut world = World::new();
    let panel = column(&mut world, stack());
    let late = child(&mut world, panel, Vec2::splat(20.0), 2);
    let early = child(&mut world, panel, Vec2::splat(20.0), -1);

    let layout = UiLayout::from_world(&world, SCREEN);
    let y = |e| layout.rect(e).map(|r| r.min.y);
    assert_eq!(y(early), Some(10.0));
    assert_eq!(y(late), Some(35.0));
}

#[test]
fn a_row_centred_both_ways() {
    let mut world = World::new();
    let panel = column(
        &mut world,
        UiStack {
            direction: Direction::Row,
            justify: Align::Center,
            align: Align::Center,
            ..stack()
        },
    );
    let a = child(&mut world, panel, Vec2::new(40.0, 20.0), 0);
    let b = child(&mut world, panel, Vec2::new(60.0, 40.0), 1);

    let layout = UiLayout::from_world(&world, SCREEN);
    // A run of 40 + 5 + 60 = 105 in 180, so it starts 37.5 in.
    let ra = layout.rect(a).expect("placed");
    let rb = layout.rect(b).expect("placed");
    assert_eq!(ra.min, Vec2::new(47.5, 140.0));
    assert_eq!(rb.min, Vec2::new(92.5, 130.0));
    assert_eq!(rb.size(), Vec2::new(60.0, 40.0));
}

#[test]
fn justified_to_the_end() {
    let mut world = World::new();
    let panel = column(
        &mut world,
        UiStack {
            justify: Align::End,
            align: Align::End,
            ..stack()
        },
    );
    let a = child(&mut world, panel, Vec2::new(40.0, 20.0), 0);
    let layout = UiLayout::from_world(&world, SCREEN);
    assert_eq!(
        layout.rect(a),
        Some(Rect::new(Vec2::new(150.0, 270.0), Vec2::new(190.0, 290.0)))
    );
}

#[test]
fn a_grandchild_resolves_inside_its_stacked_parent() {
    let mut world = World::new();
    let panel = column(&mut world, stack());
    let _first = child(&mut world, panel, Vec2::splat(20.0), 0);
    let button = child(&mut world, panel, Vec2::splat(40.0), 1);
    let label = ui(&mut world, UiRect::stretched(2.0));
    hierarchy::set_parent(&mut world, label, button).expect("parents");

    let layout = UiLayout::from_world(&world, SCREEN);
    let button_rect = layout.rect(button).expect("placed");
    assert_eq!(
        layout.rect(label),
        Some(Rect::new(
            button_rect.min + Vec2::splat(2.0),
            button_rect.max - Vec2::splat(2.0)
        ))
    );
    assert_eq!(layout.depth(label), 2);
}

#[test]
fn children_past_the_end_overflow_rather_than_vanish() {
    let mut world = World::new();
    let panel = column(&mut world, stack());
    let kids: Vec<Entity> = (0..10)
        .map(|i| child(&mut world, panel, Vec2::splat(50.0), i))
        .collect();
    let layout = UiLayout::from_world(&world, SCREEN);
    let last = layout.rect(kids[9]).expect("placed");
    assert_eq!(last.min.y, 10.0 + 9.0 * 55.0);
}

#[test]
fn a_despawned_child_closes_the_gap() {
    let mut world = World::new();
    let panel = column(&mut world, stack());
    let a = child(&mut world, panel, Vec2::splat(20.0), 0);
    let b = child(&mut world, panel, Vec2::splat(20.0), 1);
    world.despawn(a);
    let layout = UiLayout::from_world(&world, SCREEN);
    assert_eq!(layout.rect(b).map(|r| r.min.y), Some(10.0));
}

#[test]
fn an_empty_stack_or_a_non_ui_child_changes_nothing() {
    let mut world = World::new();
    let panel = column(&mut world, stack());
    let plain = world.spawn();
    world.insert(plain, SceneId::new());
    hierarchy::set_parent(&mut world, plain, panel).expect("parents");
    let layout = UiLayout::from_world(&world, SCREEN);
    assert_eq!(layout.len(), 1);
}
