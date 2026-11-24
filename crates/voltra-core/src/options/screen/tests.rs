//! The shipped options prefab opened headless, on a screen the controls
//! take input on, and worked as a player would: focus and accept, keys,
//! the cancel action.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{Name, Text, UiSlider};

use super::*;
use crate::input::{ActionMap, Binding};
use crate::window::Fullscreen;
use crate::{App, Headless, KeyCode};

const FRAME: f32 = 1.0 / 60.0;

/// The screen a test drives, shared with the tick that runs it.
type Shared = Rc<RefCell<Option<OptionsScreen>>>;

struct Harness {
    session: Headless,
    screen: Shared,
}

/// A game that opens the screen on its first frame and runs it after.
fn harness(dir: &Path) -> Harness {
    let screen: Shared = Rc::new(RefCell::new(None));
    let mut opened = false;
    let held = screen.clone();
    let config = OptionsConfig::with_actions(&[
        ("jump", "Jump"),
        ("move_left", "Move left"),
        ("move_right", "Move right"),
    ]);
    let app = App::default().with_save_dir(dir).with_update(move |tick| {
        let mut slot = held.borrow_mut();
        if !opened {
            opened = true;
            match OptionsScreen::open(tick, &config) {
                Ok(open) => *slot = Some(open),
                Err(e) => panic!("the options screen did not open: {e}"),
            }
            return;
        }
        if let Some(open) = slot.as_mut() {
            if open.update(tick) == Outcome::Closed {
                *slot = None;
            }
        }
    });
    let mut session = Headless::new(app).with_screen(Vec2::new(1280.0, 720.0));
    session.step(2, FRAME);
    Harness { session, screen }
}

fn named(world: &World, name: &str) -> Entity {
    world
        .query::<Name>()
        .find(|(_, n)| n.as_str() == name)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("nothing named {name:?}"))
}

fn label(world: &World, name: &str) -> String {
    let control = named(world, name);
    voltra_scene::hierarchy::descendants(world, control)
        .into_iter()
        .find_map(|e| world.get::<Text>(e).map(|t| t.content.clone()))
        .unwrap_or_else(|| panic!("{name:?} has no text"))
}

fn tap(session: &mut Headless, key: KeyCode) {
    session.input_mut().on_key(key, true, false);
    session.step(1, FRAME);
    session.input_mut().on_key(key, false, false);
    session.step(1, FRAME);
}

/// Focuses the control called `name` and presses accept on it.
fn press(h: &mut Harness, name: &str) {
    let control = named(h.session.world(), name);
    h.session.focus(Some(control));
    tap(&mut h.session, KeyCode::Enter);
}

#[test]
fn the_screen_lists_a_volume_per_bus_and_a_row_per_action() {
    let h = harness(&voltra_testkit::scratch_root());
    let world = h.session.world();
    for bus in ["master", "music", "sfx", "ui"] {
        assert!(label(world, &format!("Volume: {bus}")).ends_with("100%"));
    }
    assert_eq!(label(world, "Binding: jump"), "Jump: Space / Pad South");
    assert!(label(world, "Binding: move_left").starts_with("Move left: A /"));
    assert!(
        world
            .query::<Name>()
            .all(|(_, n)| n.as_str() != "Volume row" && n.as_str() != "Binding row"),
        "the templates are gone"
    );
    assert_eq!(label(world, "Display mode"), "Display: Windowed");
    assert_eq!(label(world, "Resolution"), "Resolution: 1280 × 720");
}

#[test]
fn a_rebound_key_is_shown_used_and_kept() {
    let dir = voltra_testkit::scratch_root();
    let mut h = harness(&dir);
    press(&mut h, "Binding: jump");
    assert!(label(h.session.world(), "Binding: jump").contains("press a key"));
    tap(&mut h.session, KeyCode::KeyJ);
    assert_eq!(
        label(h.session.world(), "Binding: jump"),
        "Jump: J / Pad South"
    );

    // A clash is shown on both rows, and left for the player to resolve.
    press(&mut h, "Binding: move_right");
    tap(&mut h.session, KeyCode::KeyJ);
    assert!(label(h.session.world(), "Binding: jump").ends_with("(also Move right)"));
    let row = named(h.session.world(), "Binding: jump");
    let colour = voltra_scene::hierarchy::descendants(h.session.world(), row)
        .into_iter()
        .find_map(|e| h.session.world().get::<Text>(e).map(|t| t.color));
    assert_eq!(colour, Some(CLASH_COLOR));

    press(&mut h, "Back");
    assert!(h.screen.borrow().is_none(), "Back closes the screen");
    assert!(h
        .session
        .world()
        .query::<Name>()
        .all(|(_, n)| n.as_str() != "Options"));
    let saves = crate::Saves::in_dir(&dir);
    let kept = ActionMap::load_player(&saves)
        .expect("reads")
        .expect("the controls were kept");
    assert_eq!(kept.bindings("jump")[0], Binding::Key(KeyCode::KeyJ));
    assert!(Settings::load(&saves).expect("reads").is_some());
}

#[test]
fn cancel_backs_out_of_a_rebind_then_out_of_the_screen() {
    let mut h = harness(&voltra_testkit::scratch_root());
    press(&mut h, "Binding: jump");
    tap(&mut h.session, KeyCode::Escape);
    assert_eq!(
        label(h.session.world(), "Binding: jump"),
        "Jump: Space / Pad South"
    );
    assert!(h.screen.borrow().is_some(), "the first Escape only cancels");
    tap(&mut h.session, KeyCode::Escape);
    assert!(h.screen.borrow().is_none());
}

#[test]
fn the_display_rows_reach_the_window() {
    let mut h = harness(&voltra_testkit::scratch_root());
    press(&mut h, "Display mode");
    assert_eq!(h.session.window().fullscreen(), Fullscreen::Borderless);
    assert_eq!(
        label(h.session.world(), "Display mode"),
        "Display: Fullscreen"
    );
    press(&mut h, "Display mode");
    assert_eq!(
        h.session.window().fullscreen(),
        Fullscreen::Windowed,
        "no modes headless, so no exclusive"
    );

    press(&mut h, "VSync");
    assert!(!h.session.window().vsync());
    assert_eq!(label(h.session.world(), "VSync"), "VSync: off");

    // The resolution row, stepped one size down the list by the keyboard.
    let row = named(h.session.world(), "Resolution");
    h.session.focus(Some(row));
    tap(&mut h.session, KeyCode::ArrowRight);
    let size = h.session.window().size();
    assert_ne!(size, (1280, 720));
    let screen = h.screen.borrow();
    let settings = screen.as_ref().expect("open").settings();
    assert_eq!(settings.windowed_size, Some(size));
    assert_eq!(screen.as_ref().expect("open").mode(), Fullscreen::Windowed);
}

#[test]
fn a_volume_slider_sets_its_bus() {
    let mut h = harness(&voltra_testkit::scratch_root());
    let slider = named(h.session.world(), "Volume: music");
    h.session.focus(Some(slider));
    tap(&mut h.session, KeyCode::ArrowLeft);
    let value = h
        .session
        .world()
        .get::<UiSlider>(slider)
        .expect("a slider")
        .value;
    assert!((value - 0.95).abs() < 1e-4, "{value}");
    assert_eq!(label(h.session.world(), "Volume: music"), "Music 95%");
    let screen = h.screen.borrow();
    assert!((screen.as_ref().expect("open").settings().volume("music") - 0.95).abs() < 1e-4);
}
