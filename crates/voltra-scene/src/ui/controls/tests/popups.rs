//! Dropdowns, tooltips, tabs and hidden elements through the controls.

use voltra_assets::{Fonts, DEFAULT_FONT};
use voltra_testkit::{copy_bundled_font, scratch_root};

use super::*;
use crate::ui::overlay::{overlays, Overlay};
use crate::ui::{UiDropdown, UiHidden, UiStack, UiTab, UiTabs, UiTooltip};
use crate::Name;

fn dropdown(world: &mut World, at: Vec2) -> Entity {
    let e = button(world, at);
    world.insert(e, UiDropdown::default());
    world.insert(e, Text::new(""));
    e
}

fn step(world: &mut World, state: &mut UiControls, nav: Navigation) {
    frame(world, state, Pointer::default(), nav);
}

fn nav(direction: Option<Vec2>, accept: bool, cancel: bool) -> Navigation {
    Navigation {
        direction,
        accept,
        cancel,
    }
}

#[test]
fn a_click_opens_the_list_and_a_click_on_an_item_picks_it() {
    let mut world = World::new();
    let choice = dropdown(&mut world, Vec2::new(10.0, 10.0));
    let mut state = UiControls::default();
    click(&mut world, &mut state, 50.0, 30.0);
    assert_eq!(state.open_list(), Some(choice));
    assert!(world.get::<UiDropdown>(choice).is_some_and(|d| d.open));
    assert_eq!(
        world.get::<Text>(choice).map(|t| t.content.as_str()),
        Some("One")
    );

    // The list is under the box: 40 points an item, from y = 50.
    click(&mut world, &mut state, 50.0, 50.0 + 40.0 * 2.0 + 5.0);
    assert_eq!(state.open_list(), None);
    let picked = world.get::<UiDropdown>(choice).expect("a dropdown");
    assert_eq!(picked.selected, 2);
    assert!(!picked.open);
    assert_eq!(
        world.get::<Text>(choice).map(|t| t.content.as_str()),
        Some("Three")
    );
}

#[test]
fn the_keyboard_opens_moves_and_picks_and_cancel_closes() {
    let mut world = World::new();
    let choice = dropdown(&mut world, Vec2::new(10.0, 10.0));
    let other = button(&mut world, Vec2::new(10.0, 300.0));
    let mut state = UiControls::default();
    state.focus(Some(choice));

    step(&mut world, &mut state, nav(None, true, false));
    assert_eq!(state.open_list(), Some(choice));
    step(&mut world, &mut state, nav(Some(Vec2::Y), false, false));
    assert_eq!(
        state.focused(),
        Some(choice),
        "down moves the highlight, not the focus"
    );
    step(&mut world, &mut state, nav(None, true, false));
    assert!(state.changed(choice));
    assert_eq!(world.get::<UiDropdown>(choice).map(|d| d.selected), Some(1));

    step(&mut world, &mut state, nav(None, true, false));
    step(&mut world, &mut state, nav(Some(Vec2::Y), false, false));
    step(&mut world, &mut state, nav(None, false, true));
    assert!(state.cancelled(), "the cancel closed the list");
    assert_eq!(state.open_list(), None);
    assert_eq!(
        world.get::<UiDropdown>(choice).map(|d| d.selected),
        Some(1),
        "unchanged"
    );

    step(&mut world, &mut state, nav(Some(Vec2::Y), false, false));
    assert_eq!(
        state.focused(),
        Some(other),
        "closed, down moves the focus again"
    );
}

#[test]
fn a_click_outside_closes_the_list_unchanged() {
    let mut world = World::new();
    let choice = dropdown(&mut world, Vec2::new(10.0, 10.0));
    let mut state = UiControls::default();
    click(&mut world, &mut state, 50.0, 30.0);
    click(&mut world, &mut state, 600.0, 500.0);
    assert_eq!(state.open_list(), None);
    assert!(!state.changed(choice));
}

#[test]
fn an_open_list_is_drawn_over_everything_with_its_items() {
    let root = scratch_root();
    copy_bundled_font(&root, DEFAULT_FONT);
    let fonts = Fonts::new(root);
    let mut world = World::new();
    let choice = dropdown(&mut world, Vec2::new(10.0, 10.0));
    let mut state = UiControls::default();
    click(&mut world, &mut state, 50.0, 30.0);
    let layout = UiLayout::from_world(&world, SCREEN);
    let drawn = overlays(&world, &layout, &fonts);
    let boxes = drawn
        .iter()
        .filter(|o| matches!(o, Overlay::Box { .. }))
        .count();
    let texts: Vec<&str> = drawn
        .iter()
        .filter_map(|o| match o {
            Overlay::Text { text, .. } => Some(text.content.as_str()),
            Overlay::Box { .. } => None,
        })
        .collect();
    assert_eq!(boxes, 2, "the list and the highlight");
    assert_eq!(texts, vec!["One", "Two", "Three"]);
    let _ = choice;
}

#[test]
fn a_tooltip_shows_after_its_delay_under_the_pointer_and_on_focus() {
    let mut world = World::new();
    let help = button(&mut world, Vec2::new(10.0, 10.0));
    world.insert(help, UiTooltip::new("Saves the game"));
    let plain = button(&mut world, Vec2::new(10.0, 300.0));
    let mut state = UiControls::default();
    let hover = |world: &mut World, state: &mut UiControls, y: f32, delta: f32| {
        let layout = UiLayout::from_world(world, SCREEN);
        let input = UiInput {
            pointer: Pointer {
                position: at(50.0, y),
                ..Pointer::default()
            },
            delta,
            ..UiInput::default()
        };
        state.update(world, &layout, None, &input);
    };
    let shown = |world: &World| world.get::<UiTooltip>(help).is_some_and(|t| t.shown);

    // The first frame over it starts the count.
    hover(&mut world, &mut state, 30.0, 0.3);
    hover(&mut world, &mut state, 30.0, 0.3);
    assert!(!shown(&world), "not before the delay");
    hover(&mut world, &mut state, 30.0, 0.3);
    assert!(shown(&world), "after half a second");
    hover(&mut world, &mut state, 320.0, 0.3);
    assert!(!shown(&world), "the pointer left");

    // Off the pointer's way, the focus counts instead.
    state.focus(Some(help));
    let layout = UiLayout::from_world(&world, SCREEN);
    for _ in 0..3 {
        let input = UiInput {
            delta: 0.3,
            ..UiInput::default()
        };
        state.update(&mut world, &layout, None, &input);
    }
    assert!(shown(&world));
    let _ = plain;
}

#[test]
fn a_tab_click_shows_its_page_and_reports_the_container() {
    let mut world = World::new();
    let tabs = world.spawn();
    world.insert(tabs, SceneId::new());
    world.insert(tabs, UiRect::stretched(0.0));
    world.insert(tabs, UiTabs::default());
    let a_tab = button(&mut world, Vec2::new(10.0, 10.0));
    let b_tab = button(&mut world, Vec2::new(120.0, 10.0));
    for (tab, page) in [(a_tab, "A"), (b_tab, "B")] {
        world.insert(tab, UiTab { page: page.into() });
        hierarchy::set_parent(&mut world, tab, tabs).expect("parents");
    }
    let page = |world: &mut World, name: &str| {
        let e = button(world, Vec2::new(10.0, 100.0));
        world.insert(e, Name::new(name));
        hierarchy::set_parent(world, e, tabs).expect("parents");
        e
    };
    let a = page(&mut world, "A");
    let b = page(&mut world, "B");
    let mut state = UiControls::default();
    step(&mut world, &mut state, Navigation::default());
    assert!(world.get::<UiHidden>(b).is_some());

    click(&mut world, &mut state, 150.0, 30.0);
    assert!(state.changed(tabs));
    assert!(world.get::<UiHidden>(a).is_some());
    assert!(world.get::<UiHidden>(b).is_none());
    let layout = UiLayout::from_world(&world, SCREEN);
    assert!(layout.rect(a).is_none(), "a hidden page is not laid out");
    assert!(layout.rect(b).is_some());
}

#[test]
fn a_hidden_child_takes_no_room_in_a_stack() {
    let mut world = World::new();
    let column = world.spawn();
    world.insert(column, SceneId::new());
    world.insert(
        column,
        UiRect::pinned(Vec2::ZERO, Vec2::ZERO, Vec2::new(100.0, 300.0)),
    );
    world.insert(column, UiStack::default());
    let rows: Vec<Entity> = (0..3)
        .map(|_| {
            let e = button(&mut world, Vec2::ZERO);
            hierarchy::set_parent(&mut world, e, column).expect("parents");
            e
        })
        .collect();
    let first = UiLayout::from_world(&world, SCREEN).rect(rows[0]);
    world.insert(rows[0], UiHidden);
    let layout = UiLayout::from_world(&world, SCREEN);
    assert!(layout.rect(rows[0]).is_none());
    assert_eq!(
        layout.rect(rows[1]).map(|r| r.min.y),
        first.map(|r| r.min.y),
        "the second row moves up into the first's place"
    );
}
