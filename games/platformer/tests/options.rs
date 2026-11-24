//! The options screen from the pause menu, played headless: Escape, Options,
//! the Controls tab, jump rebound to J, Back — and in the next session, with the same player
//! folder, J jumps and Space does not.

use std::path::Path;

use voltra_assets::AssetPath;
use voltra_core::{App, Headless, KeyCode};
use voltra_ecs::{Entity, World};
use voltra_platformer::components::Walker;
use voltra_platformer::{Platformer, FIRST_LEVEL};
use voltra_render::glam::Vec2;
use voltra_scene::{Name, Text, Transform};

const FRAME: f32 = 1.0 / 60.0;

/// The first level with a screen the controls take input on, keeping the
/// player's files in `dir`.
fn session(dir: &Path) -> Headless {
    let mut app = App::default()
        .with_save_dir(dir)
        .with_game(Platformer::default());
    app.gravity = Vec2::new(0.0, -9.81);
    let session = Headless::open(app, &AssetPath::new(FIRST_LEVEL).expect("a path"))
        .expect("the level loads")
        .with_screen(Vec2::new(1280.0, 720.0));
    let mut session = session;
    // Landed.
    session.step(60, FRAME);
    session
}

fn find(world: &World, name: &str) -> Option<Entity> {
    world
        .query::<Name>()
        .find(|(_, n)| n.as_str() == name)
        .map(|(e, _)| e)
}

fn named(world: &World, name: &str) -> Entity {
    find(world, name).unwrap_or_else(|| panic!("nothing named {name:?}"))
}

fn tap(session: &mut Headless, key: KeyCode) {
    session.input_mut().on_key(key, true, false);
    session.step(1, FRAME);
    session.input_mut().on_key(key, false, false);
    session.step(1, FRAME);
}

/// Focuses the control called `name` and presses Enter on it.
fn press(session: &mut Headless, name: &str) {
    let control = named(session.world(), name);
    session.focus(Some(control));
    tap(session, KeyCode::Enter);
}

fn height(session: &Headless) -> f32 {
    let (walker, _) = session
        .world()
        .query::<Walker>()
        .next()
        .expect("the level has a walker");
    session
        .world()
        .get::<Transform>(walker)
        .expect("a transform")
        .translation
        .y
}

/// How far the walker rises while `key` is held for a third of a second.
fn rise(session: &mut Headless, key: KeyCode) -> f32 {
    let start = height(session);
    let mut highest = start;
    session.input_mut().on_key(key, true, false);
    for _ in 0..20 {
        session.step(1, FRAME);
        highest = highest.max(height(session));
    }
    session.input_mut().on_key(key, false, false);
    session.step(60, FRAME);
    highest - start
}

#[test]
fn jump_rebound_in_the_options_is_jump_in_the_next_session() {
    let dir = voltra_testkit_free_scratch();
    {
        let mut first = session(&dir);
        tap(&mut first, KeyCode::Escape);
        assert!(find(first.world(), "Pause").is_some(), "Escape pauses");

        press(&mut first, "Options button");
        assert!(find(first.world(), "Pause").is_none());
        assert!(
            find(first.world(), "Options").is_some(),
            "the options screen is open"
        );

        // The controls are on their own tab.
        press(&mut first, "Controls tab");
        press(&mut first, "Binding: jump");
        tap(&mut first, KeyCode::KeyJ);
        let row = named(first.world(), "Binding: jump");
        let shown = voltra_scene::hierarchy::descendants(first.world(), row)
            .into_iter()
            .find_map(|e| first.world().get::<Text>(e).map(|t| t.content.clone()));
        assert_eq!(shown.as_deref(), Some("Jump: J / Pad South"));

        press(&mut first, "Back");
        assert!(find(first.world(), "Options").is_none());
        assert!(
            find(first.world(), "Pause").is_some(),
            "back to the pause menu"
        );
        tap(&mut first, KeyCode::Escape);
        assert!(
            find(first.world(), "Pause").is_none(),
            "and back to the game"
        );
    }

    // A new session: the game's own map, with the player's controls over it.
    let mut second = session(&dir);
    let by_space = rise(&mut second, KeyCode::Space);
    let by_j = rise(&mut second, KeyCode::KeyJ);
    assert!(by_space < 0.05, "Space no longer jumps: rose {by_space}");
    assert!(by_j > 0.5, "J jumps: rose {by_j}");
}

#[test]
fn the_level_stands_still_under_the_pause_menu() {
    let mut paused = session(&voltra_testkit_free_scratch());
    tap(&mut paused, KeyCode::Escape);
    let start = height(&paused);
    assert!(
        rise(&mut paused, KeyCode::Space) < 0.05,
        "no jump while paused"
    );
    assert!((height(&paused) - start).abs() < 0.05);
}

/// A fresh folder for the player's files. The crate has no dev-dependencies
/// (see its manifest), so not `voltra_testkit::scratch_root`.
fn voltra_testkit_free_scratch() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "voltra-platformer-options-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch folder");
    dir
}
