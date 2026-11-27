//! Toggles, sliders, text fields and scrolls, through the controls.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::{at, button, click, SCREEN};
use crate::ui::controls::{Navigation, Pointer, UiControls, UiInput};
use crate::ui::{TextInput, UiButton, UiLayout};
use crate::{hierarchy, SceneId, Sprite2D, Text, UiRect};

fn input(pointer: Pointer, navigation: Navigation, text: TextInput) -> UiInput {
    UiInput {
        pointer,
        navigation,
        text,
        delta: 0.0,
    }
}

fn run(world: &mut World, state: &mut UiControls, input: UiInput) {
    let layout = UiLayout::from_world(world, SCREEN);
    state.update(world, &layout, None, &input);
}

fn down_at(x: f32, y: f32) -> Pointer {
    Pointer {
        position: at(x, y),
        went_down: true,
        ..Pointer::default()
    }
}

fn nav(x: f32, y: f32) -> Navigation {
    Navigation {
        direction: Some(Vec2::new(x, y)),
        accept: false,
        cancel: false,
    }
}

#[test]
fn a_toggle_flips_on_each_click_and_tints_while_on() {
    let mut world = World::new();
    let check = button(&mut world, Vec2::new(10.0, 10.0));
    world.insert(check, crate::ui::UiToggle::default());
    let mut state = UiControls::default();
    click(&mut world, &mut state, 50.0, 30.0);
    assert!(state.changed(check) && state.clicked(check));
    let toggle = *world.get::<crate::ui::UiToggle>(check).expect("a toggle");
    assert!(toggle.on);
    let shown = world.get::<Sprite2D>(check).expect("a face").color;
    assert_eq!(shown, toggle.tinted(UiButton::default().hovered));
    click(&mut world, &mut state, 50.0, 30.0);
    assert!(
        !world
            .get::<crate::ui::UiToggle>(check)
            .expect("a toggle")
            .on
    );
}

fn slider(world: &mut World) -> (Entity, Entity) {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(
        e,
        UiRect::pinned(Vec2::ZERO, Vec2::new(100.0, 100.0), Vec2::new(200.0, 20.0)),
    );
    world.insert(
        e,
        crate::ui::UiSlider {
            value: 0.0,
            min: 0.0,
            max: 10.0,
            step: 1.0,
            ..crate::ui::UiSlider::default()
        },
    );
    let handle = world.spawn();
    world.insert(handle, SceneId::new());
    world.insert(
        handle,
        UiRect::pinned(Vec2::ZERO, Vec2::new(-5.0, 0.0), Vec2::new(10.0, 20.0)),
    );
    world.insert(handle, crate::ui::UiSliderPart::Handle);
    hierarchy::set_parent(world, handle, e).expect("parents");
    (e, handle)
}

#[test]
fn a_slider_follows_a_drag_and_moves_its_handle() {
    let mut world = World::new();
    let (bar, handle) = slider(&mut world);
    let mut state = UiControls::default();
    let value = |world: &World| {
        world
            .get::<crate::ui::UiSlider>(bar)
            .expect("a slider")
            .value
    };
    run(
        &mut world,
        &mut state,
        input(
            down_at(150.0, 110.0),
            Navigation::default(),
            TextInput::default(),
        ),
    );
    assert_eq!(
        value(&world),
        3.0,
        "a press sets it: a quarter along, rounded"
    );
    assert!(state.changed(bar));
    // Dragged on — off the track too — with the button still down.
    let drag = Pointer {
        position: at(260.0, 300.0),
        ..Pointer::default()
    };
    run(
        &mut world,
        &mut state,
        input(drag, Navigation::default(), TextInput::default()),
    );
    assert_eq!(value(&world), 8.0);
    let rect = world.get::<UiRect>(handle).expect("placed");
    assert_eq!(rect.anchor_min.x, 0.8, "the handle at the value");
    let up = Pointer {
        position: at(260.0, 300.0),
        went_up: true,
        ..Pointer::default()
    };
    run(
        &mut world,
        &mut state,
        input(up, Navigation::default(), TextInput::default()),
    );
    run(
        &mut world,
        &mut state,
        input(drag, Navigation::default(), TextInput::default()),
    );
    assert_eq!(value(&world), 8.0, "released, a move does nothing");
}

#[test]
fn a_focused_slider_takes_left_and_right_and_passes_up_and_down() {
    let mut world = World::new();
    let (bar, _) = slider(&mut world);
    let below = button(&mut world, Vec2::new(100.0, 200.0));
    let mut state = UiControls::default();
    state.focus(Some(bar));
    run(
        &mut world,
        &mut state,
        input(Pointer::default(), nav(1.0, 0.0), TextInput::default()),
    );
    assert_eq!(
        world
            .get::<crate::ui::UiSlider>(bar)
            .expect("a slider")
            .value,
        1.0
    );
    assert_eq!(state.focused(), Some(bar));
    run(
        &mut world,
        &mut state,
        input(Pointer::default(), nav(0.0, 1.0), TextInput::default()),
    );
    assert_eq!(state.focused(), Some(below));
}

fn field(world: &mut World, at: Vec2) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, UiRect::pinned(Vec2::ZERO, at, Vec2::new(200.0, 30.0)));
    world.insert(e, Text::new(""));
    world.insert(e, crate::ui::UiTextField::default());
    e
}

#[test]
fn a_focused_field_takes_the_typing_and_shows_it() {
    use crate::ui::text_field::{Edit, EditKey};
    let mut world = World::new();
    let name = field(&mut world, Vec2::new(10.0, 10.0));
    let mut state = UiControls::default();
    let typed = TextInput {
        typed: "Ada".to_owned(),
        ..TextInput::default()
    };
    run(
        &mut world,
        &mut state,
        input(Pointer::default(), Navigation::default(), typed.clone()),
    );
    assert_eq!(
        world.get::<Text>(name).expect("text").content,
        "",
        "not focused yet"
    );

    run(
        &mut world,
        &mut state,
        input(down_at(50.0, 20.0), Navigation::default(), typed),
    );
    assert_eq!(state.editing(&world), Some(name));
    assert!(state.changed(name));
    assert_eq!(world.get::<Text>(name).expect("text").content, "Ada");
    let enter = TextInput {
        keys: vec![
            EditKey::plain(Edit::Backspace),
            EditKey::plain(Edit::Submit),
        ],
        ..TextInput::default()
    };
    run(
        &mut world,
        &mut state,
        input(Pointer::default(), Navigation::default(), enter),
    );
    assert!(state.submitted(name));
    assert_eq!(
        world
            .get::<crate::ui::UiTextField>(name)
            .expect("a field")
            .text,
        "Ad"
    );

    // A click elsewhere ends the typing.
    run(
        &mut world,
        &mut state,
        input(
            down_at(700.0, 500.0),
            Navigation::default(),
            TextInput::default(),
        ),
    );
    assert_eq!(state.editing(&world), None);
    assert!(
        !world
            .get::<crate::ui::UiTextField>(name)
            .expect("a field")
            .focused
    );
}

#[test]
fn a_focused_field_keeps_left_and_right_for_its_caret() {
    let mut world = World::new();
    let name = field(&mut world, Vec2::new(10.0, 10.0));
    let _right = button(&mut world, Vec2::new(300.0, 10.0));
    let mut state = UiControls::default();
    state.focus(Some(name));
    run(
        &mut world,
        &mut state,
        input(Pointer::default(), nav(1.0, 0.0), TextInput::default()),
    );
    assert_eq!(state.focused(), Some(name));
}

fn list(world: &mut World, rows: i32) -> (Entity, Vec<Entity>) {
    let view = world.spawn();
    world.insert(view, SceneId::new());
    world.insert(
        view,
        UiRect::pinned(Vec2::ZERO, Vec2::new(0.0, 0.0), Vec2::new(200.0, 100.0)),
    );
    world.insert(view, crate::ui::UiScroll::default());
    world.insert(
        view,
        crate::ui::UiStack {
            spacing: 0.0,
            padding: crate::ui::Edges::default(),
            ..crate::ui::UiStack::default()
        },
    );
    let items = (0..rows)
        .map(|i| {
            let row = button(world, Vec2::ZERO);
            world.insert(
                row,
                UiRect::pinned(Vec2::ZERO, Vec2::ZERO, Vec2::new(0.0, 40.0)).with_slot(i),
            );
            hierarchy::set_parent(world, row, view).expect("parents");
            row
        })
        .collect();
    (view, items)
}

#[test]
fn the_wheel_scrolls_the_list_under_the_pointer_within_its_content() {
    let mut world = World::new();
    let (view, _) = list(&mut world, 5);
    let mut state = UiControls::default();
    let offset = |world: &World| {
        world
            .get::<crate::ui::UiScroll>(view)
            .expect("a scroll")
            .offset
            .y
    };
    let wheel = |lines: f32| Pointer {
        position: at(50.0, 50.0),
        scroll: Vec2::new(0.0, lines),
        ..Pointer::default()
    };
    run(
        &mut world,
        &mut state,
        input(wheel(-1.0), Navigation::default(), TextInput::default()),
    );
    assert_eq!(offset(&world), 40.0);
    run(
        &mut world,
        &mut state,
        input(wheel(-10.0), Navigation::default(), TextInput::default()),
    );
    assert_eq!(offset(&world), 100.0, "five rows of 40 in a view of 100");
    run(
        &mut world,
        &mut state,
        input(wheel(10.0), Navigation::default(), TextInput::default()),
    );
    assert_eq!(offset(&world), 0.0);
}

#[test]
fn moving_the_focus_down_a_list_scrolls_it_and_a_hidden_row_takes_no_click() {
    let mut world = World::new();
    let (view, rows) = list(&mut world, 5);
    let mut state = UiControls::default();
    // A press on the third row's place, below the view: nothing there.
    click(&mut world, &mut state, 50.0, 110.0);
    assert!(!rows.iter().any(|&r| state.clicked(r)));

    state.focus(Some(rows[0]));
    for _ in 0..3 {
        run(
            &mut world,
            &mut state,
            input(Pointer::default(), nav(0.0, 1.0), TextInput::default()),
        );
    }
    assert_eq!(state.focused(), Some(rows[3]));
    let offset = world
        .get::<crate::ui::UiScroll>(view)
        .expect("a scroll")
        .offset
        .y;
    assert_eq!(
        offset, 60.0,
        "the fourth row's bottom, 160, at the view's, 100"
    );
}
