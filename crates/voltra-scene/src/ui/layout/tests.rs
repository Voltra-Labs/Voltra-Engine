use super::*;
use crate::ui::UiScroll;
use crate::{SceneId, Transform2D};

fn ui(world: &mut World, rect: UiRect) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, rect);
    entity
}

const SCREEN: Vec2 = Vec2::new(800.0, 600.0);

#[test]
fn a_root_rect_resolves_against_the_screen() {
    let mut world = World::new();
    let e = ui(&mut world, UiRect::stretched(0.0));
    let layout = UiLayout::from_world(&world, SCREEN);
    assert_eq!(layout.rect(e), Some(Rect::screen(SCREEN)));
    assert_eq!(layout.depth(e), 0);
}

#[test]
fn a_child_rect_resolves_against_its_parent() {
    let mut world = World::new();
    let panel = ui(
        &mut world,
        UiRect::pinned(Vec2::ZERO, Vec2::splat(100.0), Vec2::splat(200.0)),
    );
    let label = ui(&mut world, UiRect::stretched(10.0));
    hierarchy::set_parent(&mut world, label, panel).expect("parents");

    let layout = UiLayout::from_world(&world, SCREEN);

    assert_eq!(
        layout.rect(label),
        Some(Rect::new(Vec2::splat(110.0), Vec2::splat(290.0)))
    );
    assert_eq!(layout.depth(label), 1);
}

#[test]
fn an_ancestor_without_a_rect_is_passed_over() {
    let mut world = World::new();
    let panel = ui(
        &mut world,
        UiRect::pinned(Vec2::ZERO, Vec2::splat(100.0), Vec2::splat(200.0)),
    );
    let group = world.spawn();
    world.insert(group, SceneId::new());
    world.insert(group, Transform2D::default());
    let label = ui(&mut world, UiRect::stretched(0.0));
    hierarchy::set_parent(&mut world, group, panel).expect("parents");
    hierarchy::set_parent(&mut world, label, group).expect("parents");

    let layout = UiLayout::from_world(&world, SCREEN);
    assert_eq!(layout.rect(label), layout.rect(panel));
}

#[test]
fn a_resize_moves_what_is_anchored_to_the_far_edge() {
    let mut world = World::new();
    let corner = ui(
        &mut world,
        UiRect::pinned(Vec2::ONE, Vec2::ZERO, Vec2::splat(10.0)),
    );
    let big = UiLayout::from_world(&world, SCREEN);
    let small = UiLayout::from_world(&world, Vec2::new(400.0, 300.0));
    assert_eq!(big.rect(corner).map(|r| r.max), Some(SCREEN));
    assert_eq!(
        small.rect(corner).map(|r| r.max),
        Some(Vec2::new(400.0, 300.0))
    );
}

#[test]
fn a_child_sorts_over_its_parent_at_the_same_order() {
    let mut world = World::new();
    let label = ui(&mut world, UiRect::stretched(0.0));
    let panel = ui(&mut world, UiRect::stretched(0.0));
    hierarchy::set_parent(&mut world, label, panel).expect("parents");

    let layout = UiLayout::from_world(&world, SCREEN);
    assert!(layout.draw_key(label, 0) > layout.draw_key(panel, 0));
}

#[test]
fn a_chain_deeper_than_any_stack_resolves_without_overflowing() {
    // A crafted scene file can link entities into a chain no editor
    // would build; `set_parent` would refuse it, so `Parent` is written
    // directly, the way a loaded file would.
    let mut world = World::new();
    let mut previous = ui(&mut world, UiRect::stretched(0.0));
    let mut last = previous;
    for _ in 0..20_000 {
        last = ui(&mut world, UiRect::stretched(0.0));
        let id = *world.get::<SceneId>(previous).expect("an id");
        world.insert(last, hierarchy::Parent::resolved(id, previous));
        previous = last;
    }

    let layout = UiLayout::from_world(&world, SCREEN);
    assert_eq!(layout.rect(last), Some(Rect::screen(SCREEN)));
}

#[test]
fn a_parent_cycle_resolves_rather_than_looping() {
    let mut world = World::new();
    let a = ui(&mut world, UiRect::stretched(1.0));
    let b = ui(&mut world, UiRect::stretched(1.0));
    let (id_a, id_b) = (
        *world.get::<SceneId>(a).expect("an id"),
        *world.get::<SceneId>(b).expect("an id"),
    );
    world.insert(a, hierarchy::Parent::resolved(id_b, b));
    world.insert(b, hierarchy::Parent::resolved(id_a, a));

    let layout = UiLayout::from_world(&world, SCREEN);
    assert!(layout.rect(a).is_some() && layout.rect(b).is_some());
}

#[test]
fn an_entity_without_a_rect_is_not_in_the_layout() {
    let mut world = World::new();
    let plain = world.spawn();
    let layout = UiLayout::from_world(&world, SCREEN);
    assert_eq!(layout.rect(plain), None);
    assert!(layout.is_empty());
}

#[test]
fn a_scroll_moves_and_masks_its_content() {
    let mut world = World::new();
    let view = ui(
        &mut world,
        UiRect::pinned(Vec2::ZERO, Vec2::splat(100.0), Vec2::splat(100.0)),
    );
    world.insert(
        view,
        UiScroll {
            offset: Vec2::new(0.0, 30.0),
            ..UiScroll::default()
        },
    );
    let item = ui(
        &mut world,
        UiRect::pinned(Vec2::ZERO, Vec2::new(0.0, 80.0), Vec2::new(50.0, 40.0)),
    );
    hierarchy::set_parent(&mut world, item, view).expect("parents");
    let inner = ui(&mut world, UiRect::stretched(5.0));
    hierarchy::set_parent(&mut world, inner, item).expect("parents");

    let layout = UiLayout::from_world(&world, SCREEN);
    assert_eq!(
        layout.rect(item),
        Some(Rect::new(Vec2::new(100.0, 150.0), Vec2::new(150.0, 190.0)))
    );
    let mask = Rect::new(Vec2::splat(100.0), Vec2::splat(200.0));
    assert_eq!(layout.clip(item), Some(mask));
    assert_eq!(layout.clip(inner), Some(mask), "inherited");
    assert_eq!(layout.scroll_of(inner), Some(view));
    assert_eq!(layout.clip(view), None, "the view itself is not masked");
    assert_eq!(
        layout.content(view).map(|r| r.max.y),
        Some(190.0),
        "the content reaches the item's bottom"
    );
}

#[test]
fn nested_scrolls_mask_by_both() {
    let mut world = World::new();
    let outer = ui(
        &mut world,
        UiRect::pinned(Vec2::ZERO, Vec2::ZERO, Vec2::splat(100.0)),
    );
    world.insert(outer, UiScroll::default());
    let inner = ui(
        &mut world,
        UiRect::pinned(Vec2::ZERO, Vec2::splat(50.0), Vec2::splat(100.0)),
    );
    world.insert(inner, UiScroll::default());
    hierarchy::set_parent(&mut world, inner, outer).expect("parents");
    let leaf = ui(&mut world, UiRect::stretched(0.0));
    hierarchy::set_parent(&mut world, leaf, inner).expect("parents");

    let layout = UiLayout::from_world(&world, SCREEN);
    assert_eq!(
        layout.clip(leaf),
        Some(Rect::new(Vec2::splat(50.0), Vec2::splat(100.0)))
    );
    assert_eq!(layout.scroll_of(leaf), Some(inner));
    assert_eq!(layout.scroll_of(inner), Some(outer));
    assert_eq!(
        layout.visible(leaf),
        Some(Rect::new(Vec2::splat(50.0), Vec2::splat(100.0)))
    );
}

#[test]
fn a_stack_inside_a_scroll_scrolls() {
    let mut world = World::new();
    let list = ui(
        &mut world,
        UiRect::pinned(Vec2::ZERO, Vec2::ZERO, Vec2::new(100.0, 50.0)),
    );
    world.insert(
        list,
        UiScroll {
            offset: Vec2::new(0.0, 20.0),
            ..UiScroll::default()
        },
    );
    world.insert(
        list,
        crate::ui::UiStack {
            spacing: 0.0,
            padding: crate::ui::Edges::default(),
            ..crate::ui::UiStack::default()
        },
    );
    let rows: Vec<Entity> = (0..3)
        .map(|i| {
            let row = ui(
                &mut world,
                UiRect::pinned(Vec2::ZERO, Vec2::ZERO, Vec2::new(0.0, 30.0)).with_slot(i),
            );
            hierarchy::set_parent(&mut world, row, list).expect("parents");
            row
        })
        .collect();
    let layout = UiLayout::from_world(&world, SCREEN);
    assert_eq!(layout.rect(rows[0]).map(|r| r.min.y), Some(-20.0));
    assert_eq!(layout.rect(rows[2]).map(|r| r.max.y), Some(70.0));
    assert_eq!(layout.visible(rows[0]).map(|r| r.min.y), Some(0.0));
}
