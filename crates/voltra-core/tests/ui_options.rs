//! An options screen — a volume slider, a fullscreen toggle, a name field
//! and a scrolling list of saves — worked by the mouse, the keyboard and a
//! pad, through the engine's own input: the cursor and buttons, the keys
//! and actions of the default input map, the pad's D-pad and South, and
//! typing, all as a window would feed them, on a headless screen.

use voltra_core::input::PadButton;
use voltra_core::{App, Headless, KeyCode, MouseButton};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::ui::{Edges, UiStack};
use voltra_scene::{
    hierarchy, Align, Name, SceneId, Sprite2D, Text, UiButton, UiControls, UiRect, UiScroll,
    UiSlider, UiSliderPart, UiTextField, UiToggle,
};

const FRAME: f32 = 1.0 / 60.0;
const SCREEN: Vec2 = Vec2::new(400.0, 500.0);

struct Screen {
    volume: Entity,
    fullscreen: Entity,
    name: Entity,
    saves: Entity,
    rows: Vec<Entity>,
}

fn spawn(world: &mut World, name: &str, rect: UiRect) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, Name::new(name));
    world.insert(e, rect);
    e
}

fn label(world: &mut World, parent: Entity, words: &str) {
    let e = spawn(world, "Label", UiRect::stretched(4.0));
    world.insert(
        e,
        Text {
            size: 14.0,
            align: Align::Center,
            ..Text::new(words)
        },
    );
    hierarchy::set_parent(world, e, parent).expect("parents");
}

fn tall(height: f32, slot: i32) -> UiRect {
    UiRect::pinned(Vec2::ZERO, Vec2::ZERO, Vec2::new(0.0, height)).with_slot(slot)
}

/// The screen, stacked down the window 10 points in: the slider at
/// y 10..30, the toggle 40..70, the field 80..110, and the list of saves
/// 120..240 — six rows of 40, three in view.
fn options(world: &mut World) -> Screen {
    let panel = spawn(world, "Options", UiRect::stretched(0.0));
    world.insert(
        panel,
        UiStack {
            spacing: 10.0,
            padding: Edges::uniform(10.0),
            ..UiStack::default()
        },
    );

    let volume = spawn(world, "Volume", tall(20.0, 0));
    world.insert(volume, Sprite2D::new([0.1, 0.1, 0.1, 1.0]));
    world.insert(
        volume,
        UiSlider {
            value: 50.0,
            min: 0.0,
            max: 100.0,
            step: 10.0,
            ..UiSlider::default()
        },
    );
    let handle = spawn(
        world,
        "Handle",
        UiRect::pinned(Vec2::ZERO, Vec2::new(-5.0, 0.0), Vec2::new(10.0, 20.0)),
    );
    world.insert(handle, Sprite2D::default());
    world.insert(handle, UiSliderPart::Handle);
    hierarchy::set_parent(world, handle, volume).expect("parents");

    let fullscreen = spawn(world, "Fullscreen", tall(30.0, 1));
    world.insert(fullscreen, Sprite2D::default());
    world.insert(fullscreen, UiButton::default());
    world.insert(fullscreen, UiToggle::default());
    label(world, fullscreen, "Fullscreen");

    let name = spawn(world, "Name", tall(30.0, 2));
    world.insert(name, Sprite2D::new([0.05, 0.05, 0.05, 1.0]));
    world.insert(name, Text::new(""));
    world.insert(
        name,
        UiTextField {
            placeholder: "Your name".to_owned(),
            ..UiTextField::default()
        },
    );

    let saves = spawn(world, "Saves", tall(120.0, 3));
    world.insert(saves, Sprite2D::new([0.1, 0.1, 0.12, 1.0]));
    world.insert(saves, UiScroll::default());
    world.insert(
        saves,
        UiStack {
            spacing: 0.0,
            padding: Edges::default(),
            ..UiStack::default()
        },
    );
    let rows = (0..6)
        .map(|i| {
            let row = spawn(world, &format!("Save {i}"), tall(40.0, i));
            world.insert(row, Sprite2D::default());
            world.insert(row, UiButton::default());
            label(world, row, &format!("Save {i}"));
            hierarchy::set_parent(world, row, saves).expect("parents");
            row
        })
        .collect();

    for e in [volume, fullscreen, name, saves] {
        hierarchy::set_parent(world, e, panel).expect("parents");
    }
    Screen {
        volume,
        fullscreen,
        name,
        saves,
        rows,
    }
}

fn session() -> (Headless, Screen) {
    let mut session = Headless::new(App::default()).with_screen(SCREEN);
    let screen = options(session.world_mut());
    session.step(1, FRAME);
    (session, screen)
}

fn click(session: &mut Headless, x: f32, y: f32) {
    session.input_mut().on_cursor_moved(Vec2::new(x, y));
    session.input_mut().on_mouse_button(MouseButton::Left, true);
    session.step(1, FRAME);
    session
        .input_mut()
        .on_mouse_button(MouseButton::Left, false);
    session.step(1, FRAME);
}

/// One press of `action` through its key, then its release: the controls
/// as the press left them, since the frame's clicks are that frame's.
fn tap(session: &mut Headless, action: &str) -> UiControls {
    assert!(session.press(action), "{action} is bound");
    session.step(1, FRAME);
    let pressed = session.controls().clone();
    session.release(action);
    session.step(1, FRAME);
    pressed
}

/// One press of a pad button, then its release, as [`tap`] answers.
fn pad(session: &mut Headless, button: PadButton) -> UiControls {
    session.input_mut().on_pad_button(0, button, true);
    session.step(1, FRAME);
    let pressed = session.controls().clone();
    session.input_mut().on_pad_button(0, button, false);
    session.step(1, FRAME);
    pressed
}

fn type_key(session: &mut Headless, code: KeyCode, text: Option<&str>) {
    session.input_mut().on_key(code, true, false);
    session.input_mut().typing_mut().on_key(code, true, text);
    session.step(1, FRAME);
    session.input_mut().on_key(code, false, false);
    session.step(1, FRAME);
}

fn volume(session: &Headless, screen: &Screen) -> f32 {
    session
        .world()
        .get::<UiSlider>(screen.volume)
        .expect("a slider")
        .value
}

fn toggled(session: &Headless, screen: &Screen) -> bool {
    session
        .world()
        .get::<UiToggle>(screen.fullscreen)
        .expect("a toggle")
        .on
}

fn typed(session: &Headless, screen: &Screen) -> String {
    session
        .world()
        .get::<UiTextField>(screen.name)
        .expect("a field")
        .text
        .clone()
}

fn scrolled(session: &Headless, screen: &Screen) -> f32 {
    session
        .world()
        .get::<UiScroll>(screen.saves)
        .expect("a scroll")
        .offset
        .y
}

#[test]
fn by_the_mouse() {
    let (mut session, screen) = session();

    // The slider: pressed 80% along, dragged back to 10%, let go.
    session.input_mut().on_cursor_moved(Vec2::new(314.0, 20.0));
    session.input_mut().on_mouse_button(MouseButton::Left, true);
    session.step(1, FRAME);
    assert_eq!(volume(&session, &screen), 80.0);
    assert!(session.controls().changed(screen.volume));
    session.input_mut().on_cursor_moved(Vec2::new(48.0, 300.0));
    session.step(1, FRAME);
    assert_eq!(
        volume(&session, &screen),
        10.0,
        "dragged, off the track too"
    );
    session
        .input_mut()
        .on_mouse_button(MouseButton::Left, false);
    session.step(1, FRAME);

    // The toggle, by its label.
    click(&mut session, 200.0, 55.0);
    assert!(toggled(&session, &screen));

    // The field: clicked, typed in.
    click(&mut session, 200.0, 95.0);
    assert_eq!(
        session.controls().editing(session.world()),
        Some(screen.name)
    );
    for (code, text) in [
        (KeyCode::KeyA, "A"),
        (KeyCode::KeyD, "d"),
        (KeyCode::KeyA, "a"),
    ] {
        type_key(&mut session, code, Some(text));
    }
    assert_eq!(typed(&session, &screen), "Ada");
    let shown = session.world().get::<Text>(screen.name).expect("text");
    assert_eq!(shown.content, "Ada");

    // The list: the wheel scrolls it a row, and the row now in its middle
    // takes a click; a row scrolled out of view does not.
    session.input_mut().on_cursor_moved(Vec2::new(200.0, 180.0));
    session.input_mut().on_scroll(-1.0);
    session.step(1, FRAME);
    assert_eq!(scrolled(&session, &screen), 40.0);
    assert_eq!(
        session.controls().editing(session.world()),
        Some(screen.name),
        "the wheel leaves the typing alone"
    );
    session.input_mut().on_cursor_moved(Vec2::new(200.0, 180.0));
    session.input_mut().on_mouse_button(MouseButton::Left, true);
    session.step(1, FRAME);
    session
        .input_mut()
        .on_mouse_button(MouseButton::Left, false);
    session.step(1, FRAME);
    assert!(
        session.controls().clicked(screen.rows[2]),
        "Save 2 is at 160..200 now"
    );
    session.input_mut().on_scroll(-10.0);
    session.step(1, FRAME);
    assert_eq!(
        scrolled(&session, &screen),
        120.0,
        "no further than the last row"
    );
}

#[test]
fn by_the_keyboard() {
    let (mut session, screen) = session();
    session.focus(Some(screen.volume));

    tap(&mut session, "ui_right");
    assert_eq!(volume(&session, &screen), 60.0, "a step along the slider");
    tap(&mut session, "ui_down");
    assert_eq!(session.controls().focused(), Some(screen.fullscreen));
    tap(&mut session, "ui_accept");
    assert!(toggled(&session, &screen));

    tap(&mut session, "ui_down");
    assert_eq!(
        session.controls().editing(session.world()),
        Some(screen.name)
    );
    for (code, text) in [
        (KeyCode::KeyB, "B"),
        (KeyCode::KeyO, "o"),
        (KeyCode::KeyB, "b"),
    ] {
        type_key(&mut session, code, Some(text));
    }
    // The arrows move the caret in a field, not the focus.
    type_key(&mut session, KeyCode::ArrowLeft, None);
    assert_eq!(session.controls().focused(), Some(screen.name));
    type_key(&mut session, KeyCode::Backspace, None);
    assert_eq!(typed(&session, &screen), "Bb");
    session.input_mut().on_key(KeyCode::Enter, true, false);
    session
        .input_mut()
        .typing_mut()
        .on_key(KeyCode::Enter, true, Some("\r"));
    session.step(1, FRAME);
    assert!(session.controls().submitted(screen.name));
    session.input_mut().on_key(KeyCode::Enter, false, false);
    session.step(1, FRAME);

    // Down into the list and along it: it scrolls to keep the focus seen.
    tap(&mut session, "ui_down");
    assert_eq!(session.controls().focused(), Some(screen.rows[0]));
    for _ in 0..3 {
        tap(&mut session, "ui_down");
    }
    assert_eq!(session.controls().focused(), Some(screen.rows[3]));
    assert_eq!(
        scrolled(&session, &screen),
        40.0,
        "Save 3 brought into view"
    );
    let pressed = tap(&mut session, "ui_accept");
    assert!(pressed.clicked(screen.rows[3]));
}

#[test]
fn by_a_pad() {
    let (mut session, screen) = session();
    session.focus(Some(screen.rows[5]));
    session.step(1, FRAME);
    // Moving the focus there scrolled the list to its end.
    pad(&mut session, PadButton::DPadUp);
    assert_eq!(session.controls().focused(), Some(screen.rows[4]));
    assert_eq!(scrolled(&session, &screen), 120.0);
    let pressed = pad(&mut session, PadButton::South);
    assert!(pressed.clicked(screen.rows[4]));

    // Up the list, which scrolls back, then out of it to the field, the
    // toggle and the slider.
    for _ in 0..4 {
        pad(&mut session, PadButton::DPadUp);
    }
    assert_eq!(session.controls().focused(), Some(screen.rows[0]));
    assert_eq!(scrolled(&session, &screen), 0.0);
    pad(&mut session, PadButton::DPadUp);
    assert_eq!(session.controls().focused(), Some(screen.name));
    pad(&mut session, PadButton::DPadUp);
    pad(&mut session, PadButton::South);
    assert!(toggled(&session, &screen));
    pad(&mut session, PadButton::DPadUp);
    assert_eq!(session.controls().focused(), Some(screen.volume));
    pad(&mut session, PadButton::DPadLeft);
    pad(&mut session, PadButton::DPadLeft);
    assert_eq!(volume(&session, &screen), 30.0);
}
