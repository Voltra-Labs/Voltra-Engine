//! The platformer loaded as the editor loads it: from its `cdylib`, checked
//! by its stamp, its components registered through the library and its game
//! played from it.
//!
//! The library is the one cargo built for this very test run, so it was
//! compiled with exactly the features the test was — which is why this
//! crate has no dev-dependencies.

use std::path::Path;

use voltra_assets::AssetPath;
use voltra_core::game::built::open_built;
use voltra_core::game::library::GameLibrary;
use voltra_core::{App, Headless};
use voltra_platformer::FIRST_LEVEL;
use voltra_scene::{ComponentRegistry, Text};

const FRAME: f32 = 1.0 / 60.0;

/// The library cargo built for this run, opened as the editor opens it:
/// of the copies cargo has left (one per feature set it built the game
/// with), the newest this build's stamp accepts.
fn built_library() -> GameLibrary {
    open_built(
        Path::new(env!("CARGO_BIN_EXE_voltra-platformer")),
        "voltra-platformer",
    )
    .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn the_library_is_accepted_and_registers_the_games_components() {
    let library = built_library();
    let mut registry = ComponentRegistry::with_defaults();
    library.register(&mut registry);

    for name in [
        "Walker",
        "WalkerMotion",
        "Coin",
        "Goal",
        "CoinCounter",
        "SaveButton",
        "FullscreenButton",
    ] {
        assert!(registry.names().any(|n| n == name), "{name} registered");
    }
}

#[test]
fn the_game_from_the_library_takes_a_coin() {
    let library = built_library();
    let mut app = App::default().with_game(library.new_game());
    app.gravity = voltra_render::glam::Vec2::new(0.0, -9.81);
    let mut session = Headless::open(app, &AssetPath::new(FIRST_LEVEL).expect("a path"))
        .expect("the level loads");

    session.step(30, FRAME);
    session.press("move_right");
    session.step(150, FRAME);

    // The counter is found by the library's `CoinCounter`, whose `TypeId`
    // is the library's own: the host reads the text it wrote.
    let texts: Vec<String> = session
        .world()
        .query::<Text>()
        .map(|(_, text)| text.content.clone())
        .collect();
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("Coins 1 / 3") || t.starts_with("Coins 2 / 3")),
        "{texts:?}"
    );
}

#[test]
fn reloading_the_library_mid_play_keeps_the_world_and_the_count() {
    let first = built_library();
    let mut app = App::default().with_game(first.new_game());
    app.gravity = voltra_render::glam::Vec2::new(0.0, -9.81);
    let mut session = Headless::open(app, &AssetPath::new(FIRST_LEVEL).expect("a path"))
        .expect("the level loads");
    session.step(30, FRAME);
    session.press("move_right");
    let mut frames = 0;
    while !counter_reads(&session, "Coins 1 / 3") && frames < 240 {
        session.step(1, FRAME);
        frames += 1;
    }
    assert!(
        counter_reads(&session, "Coins 1 / 3"),
        "took the first coin"
    );
    let walker_before = walker_x(&session);

    // A second load of the library is a fresh copy: what a rebuild gives.
    let second = built_library();
    session
        .swap_game(&second)
        .expect("the scene reads through the new library");
    // SAFETY: the swap rebuilt the world and the registry through the
    // second library and dropped the first's game.
    unsafe { first.close() };

    assert!(
        (walker_x(&session) - walker_before).abs() < 1e-4,
        "the walker stayed put"
    );
    assert!(
        counter_reads(&session, "Coins 1 / 3"),
        "the count lived through it"
    );
    // The new game's turns move the walker on: up to the ledge's foot.
    session.press("move_right");
    session.step(60, FRAME);
    assert!(
        walker_x(&session) > walker_before + 0.5,
        "the new code walks it: {} from {walker_before}",
        walker_x(&session)
    );
}

fn counter_reads(session: &Headless, words: &str) -> bool {
    session
        .world()
        .query::<Text>()
        .any(|(_, text)| text.content == words)
}

fn walker_x(session: &Headless) -> f32 {
    let named = |name: &str| {
        session
            .world()
            .query::<voltra_scene::Name>()
            .find(|(_, n)| n.as_str() == name)
            .map(|(e, _)| e)
    };
    let walker = named("Walker").expect("the level has a walker");
    session
        .world()
        .get::<voltra_scene::Transform2D>(walker)
        .expect("placed")
        .translation
        .x
}
