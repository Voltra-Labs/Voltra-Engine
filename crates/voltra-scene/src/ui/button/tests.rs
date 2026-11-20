use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::*;
use crate::{hierarchy, SceneId, Sprite, Text, UiRect};

const SCREEN: Vec2 = Vec2::new(800.0, 600.0);

/// A 100 × 40 button with its top-left at `at`.
fn button(world: &mut World, at: Vec2) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, UiRect::pinned(Vec2::ZERO, at, Vec2::new(100.0, 40.0)));
    world.insert(e, Sprite::default());
    world.insert(e, UiButton::default());
    e
}

fn frame(world: &mut World, state: &mut UiButtons, pointer: Pointer, nav: Navigation) {
    let layout = UiLayout::from_world(world, SCREEN);
    state.update(world, &layout, pointer, nav);
}

fn at(x: f32, y: f32) -> Option<Vec2> {
    Some(Vec2::new(x, y))
}

fn click(world: &mut World, state: &mut UiButtons, x: f32, y: f32) {
    let down = Pointer {
        position: at(x, y),
        went_down: true,
        went_up: false,
    };
    frame(world, state, down, Navigation::default());
    let up = Pointer {
        position: at(x, y),
        went_down: false,
        went_up: true,
    };
    frame(world, state, up, Navigation::default());
}

#[test]
fn a_press_and_release_over_a_button_clicks_it() {
    let mut world = World::new();
    let ok = button(&mut world, Vec2::new(10.0, 10.0));
    let mut state = UiButtons::default();
    click(&mut world, &mut state, 50.0, 30.0);
    assert!(state.clicked(ok));
    assert_eq!(state.focused(), Some(ok), "a click focuses");
}

#[test]
fn the_click_lasts_one_frame() {
    let mut world = World::new();
    let ok = button(&mut world, Vec2::new(10.0, 10.0));
    let mut state = UiButtons::default();
    click(&mut world, &mut state, 50.0, 30.0);
    frame(
        &mut world,
        &mut state,
        Pointer::default(),
        Navigation::default(),
    );
    assert!(!state.clicked(ok));
}

#[test]
fn releasing_somewhere_else_cancels_the_click() {
    let mut world = World::new();
    let ok = button(&mut world, Vec2::new(10.0, 10.0));
    let mut state = UiButtons::default();
    let down = Pointer {
        position: at(50.0, 30.0),
        went_down: true,
        went_up: false,
    };
    frame(&mut world, &mut state, down, Navigation::default());
    let up = Pointer {
        position: at(500.0, 500.0),
        went_down: false,
        went_up: true,
    };
    frame(&mut world, &mut state, up, Navigation::default());
    assert!(!state.clicked(ok));
}

#[test]
fn a_click_on_the_label_reaches_the_button() {
    let mut world = World::new();
    let ok = button(&mut world, Vec2::new(10.0, 10.0));
    let label = world.spawn();
    world.insert(label, SceneId::new());
    world.insert(label, UiRect::stretched(0.0));
    world.insert(label, Text::new("OK"));
    hierarchy::set_parent(&mut world, label, ok).expect("parents");
    let mut state = UiButtons::default();
    click(&mut world, &mut state, 50.0, 30.0);
    assert!(state.clicked(ok));
}

#[test]
fn a_disabled_button_takes_no_click_and_shows_its_colour() {
    let mut world = World::new();
    let ok = button(&mut world, Vec2::new(10.0, 10.0));
    world.get_mut::<UiButton>(ok).expect("button").disabled = true;
    let mut state = UiButtons::default();
    click(&mut world, &mut state, 50.0, 30.0);
    assert!(!state.clicked(ok));
    assert_eq!(
        world.get::<Sprite>(ok).map(|s| s.color),
        Some(UiButton::default().disabled_color)
    );
}

#[test]
fn hover_tints_and_leaving_restores() {
    let mut world = World::new();
    let ok = button(&mut world, Vec2::new(10.0, 10.0));
    let mut state = UiButtons::default();
    let over = Pointer {
        position: at(50.0, 30.0),
        ..Pointer::default()
    };
    frame(&mut world, &mut state, over, Navigation::default());
    assert!(state.hovered(ok));
    assert_eq!(
        world.get::<Sprite>(ok).map(|s| s.color),
        Some(UiButton::default().hovered)
    );
    frame(
        &mut world,
        &mut state,
        Pointer::default(),
        Navigation::default(),
    );
    assert_eq!(
        world.get::<Sprite>(ok).map(|s| s.color),
        Some(UiButton::default().normal)
    );
}

#[test]
fn navigation_starts_top_left_and_moves_to_the_nearest_ahead() {
    // A column of two, and one off to the right of the first.
    let mut world = World::new();
    let first = button(&mut world, Vec2::new(10.0, 10.0));
    let below = button(&mut world, Vec2::new(10.0, 70.0));
    let right = button(&mut world, Vec2::new(200.0, 10.0));
    let mut state = UiButtons::default();
    let go = |dir: Vec2| Navigation {
        direction: Some(dir),
        accept: false,
    };

    frame(&mut world, &mut state, Pointer::default(), go(Vec2::Y));
    assert_eq!(
        state.focused(),
        Some(first),
        "no focus yet: the top-left one"
    );
    frame(&mut world, &mut state, Pointer::default(), go(Vec2::Y));
    assert_eq!(state.focused(), Some(below), "down is y+ on screen");
    frame(&mut world, &mut state, Pointer::default(), go(-Vec2::Y));
    frame(&mut world, &mut state, Pointer::default(), go(Vec2::X));
    assert_eq!(state.focused(), Some(right));
    frame(&mut world, &mut state, Pointer::default(), go(Vec2::X));
    assert_eq!(
        state.focused(),
        Some(right),
        "nothing further right: it stays"
    );
}

#[test]
fn accept_clicks_the_focused_button() {
    let mut world = World::new();
    let ok = button(&mut world, Vec2::new(10.0, 10.0));
    let mut state = UiButtons::default();
    state.focus(Some(ok));
    let accept = Navigation {
        direction: None,
        accept: true,
    };
    frame(&mut world, &mut state, Pointer::default(), accept);
    assert!(state.clicked(ok));
}

#[test]
fn a_despawned_focus_is_dropped() {
    let mut world = World::new();
    let ok = button(&mut world, Vec2::new(10.0, 10.0));
    let mut state = UiButtons::default();
    state.focus(Some(ok));
    world.despawn(ok);
    frame(
        &mut world,
        &mut state,
        Pointer::default(),
        Navigation::default(),
    );
    assert_eq!(state.focused(), None);
}
