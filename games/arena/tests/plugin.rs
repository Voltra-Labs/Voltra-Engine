//! The game loaded as the editor loads it — from its library, checked by
//! its stamp — and its first scene played headless with a key held.

use std::path::PathBuf;

use arena::FIRST_SCENE;
use voltra_assets::AssetPath;
use voltra_core::game::library::{library_file, GameLibrary};
use voltra_core::{App, Headless};
use voltra_scene::{Name, Transform2D};

/// The library cargo built for this run: beside the binary after a
/// `cargo build`, beside the `rlib` the test links after a `cargo test`.
fn built_library() -> PathBuf {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_arena"));
    let profile = binary.parent().expect("the binary is in a folder");
    let file = library_file("arena");
    let mut found = vec![profile.join(&file), profile.join("deps").join(&file)];
    if let Ok(units) = std::fs::read_dir(profile.join("build").join("arena")) {
        found.extend(
            units
                .flatten()
                .map(|unit| unit.path().join("out").join(&file)),
        );
    }
    found
        .into_iter()
        .filter_map(|path| Some((std::fs::metadata(&path).ok()?.modified().ok()?, path)))
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
        .unwrap_or_else(|| panic!("cargo built {file} under {}", profile.display()))
}

#[test]
fn the_editor_can_load_and_play_the_game() {
    let library = GameLibrary::open(&built_library()).expect("a library this build can run");
    let app = App::default().with_game(library.new_game());
    let scene = AssetPath::new(FIRST_SCENE).expect("a path");
    let mut session = Headless::open(app, &scene).expect("the first scene loads");

    // Alone: the game spawns this run's player at its first turn.
    session.step(1, 1.0 / 60.0);
    let (player, _) = session
        .world()
        .query::<Name>()
        .find(|(_, name)| name.as_str() == "Player")
        .expect("the game spawned a player");
    let x = |session: &Headless| {
        session
            .world()
            .get::<Transform2D>(player)
            .expect("placed")
            .translation
            .x
    };
    let from = x(&session);
    assert!(
        session.press("move_right"),
        "the input map binds move_right"
    );
    session.step(30, 1.0 / 60.0);
    assert!(x(&session) > from + 1.0, "the player walked right");
}
