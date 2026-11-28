//! The game loaded as the editor loads it — from its library, checked by
//! its stamp — and its first scene played headless with a key held.

use std::path::Path;

use breakout::FIRST_SCENE;
use voltra_assets::AssetPath;
use voltra_core::game::built::open_built;
use voltra_core::game::library::GameLibrary;
use voltra_core::{App, Headless};
use voltra_scene::{Name, Transform2D};

/// The library cargo built for this run, opened as the editor opens it:
/// of the copies cargo has left (one per feature set it built the game
/// with), the newest this build's stamp accepts.
fn built_library() -> GameLibrary {
    open_built(Path::new(env!("CARGO_BIN_EXE_breakout")), "breakout")
        .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn the_editor_can_load_and_play_the_game() {
    let library = built_library();
    let app = App::default().with_game(library.new_game());
    let scene = AssetPath::new(FIRST_SCENE).expect("a path");
    let mut session = Headless::open(app, &scene).expect("the first scene loads");

    assert!(
        session.press("move_right"),
        "the input map binds move_right"
    );
    session.step(30, 1.0 / 60.0);

    let (paddle, _) = session
        .world()
        .query::<Name>()
        .find(|(_, name)| name.as_str() == "Paddle")
        .expect("the scene has a paddle");
    let at = session
        .world()
        .get::<Transform2D>(paddle)
        .expect("placed")
        .translation;
    assert!(at.x > 1.0, "the paddle moved right: {at}");
}
