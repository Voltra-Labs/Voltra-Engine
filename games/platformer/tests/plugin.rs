//! The platformer loaded as the editor loads it: from its `cdylib`, checked
//! by its stamp, its components registered through the library and its game
//! played from it.
//!
//! The library is the one cargo built for this very test run, so it was
//! compiled with exactly the features the test was — which is why this
//! crate has no dev-dependencies.

use std::path::PathBuf;

use voltra_assets::AssetPath;
use voltra_core::game::library::{library_file, GameLibrary};
use voltra_core::{App, Headless};
use voltra_platformer::FIRST_LEVEL;
use voltra_scene::{ComponentRegistry, Text};

const FRAME: f32 = 1.0 / 60.0;

/// The library cargo built for this run.
///
/// `cargo build` copies it beside the crate's binary — whose path cargo
/// hands integration tests — but `cargo test` leaves it where it compiled
/// it, beside the `rlib` the test links: under `deps/`, or under
/// `build/voltra-platformer/<hash>/out/` in cargo's newer layout. Of those
/// found, the newest is this run's.
fn built_library() -> PathBuf {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_voltra-platformer"));
    let profile = binary.parent().expect("the binary is in a folder");
    let file = library_file("voltra-platformer");
    let mut found: Vec<PathBuf> = vec![profile.join(&file), profile.join("deps").join(&file)];
    if let Ok(units) = std::fs::read_dir(profile.join("build").join("voltra-platformer")) {
        found.extend(
            units
                .flatten()
                .map(|unit| unit.path().join("out").join(&file)),
        );
    }
    found
        .into_iter()
        .filter_map(|path| {
            let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
            Some((modified, path))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
        .unwrap_or_else(|| panic!("cargo built {file} under {}", profile.display()))
}

#[test]
fn the_library_is_accepted_and_registers_the_games_components() {
    let library = GameLibrary::open(&built_library()).expect("a library this build can run");
    let mut registry = ComponentRegistry::with_defaults();
    library.register(&mut registry);

    for name in ["Walker", "Coin", "Goal", "CoinCounter", "SaveButton"] {
        assert!(registry.names().any(|n| n == name), "{name} registered");
    }
}

#[test]
fn the_game_from_the_library_takes_a_coin() {
    let library = GameLibrary::open(&built_library()).expect("a library this build can run");
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
