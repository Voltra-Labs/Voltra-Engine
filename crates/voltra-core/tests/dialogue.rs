//! A dialogue box — a panel whose text wraps to it, naming a string-table
//! key — switched from English to Spanish while the game runs, by a button
//! the game's tick answers; a string a table lacks shows its key.

use std::path::Path;

use voltra_assets::DEFAULT_FONT;
use voltra_core::{App, Headless, MouseButton};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::text::ui_px;
use voltra_scene::{hierarchy, Name, SceneId, Sprite2D, Text, UiButton, UiLayout, UiRect};
use voltra_testkit::{copy_bundled_font, scratch_root};

const FRAME: f32 = 1.0 / 60.0;
const SCREEN: Vec2 = Vec2::new(640.0, 360.0);
/// The box's text area: 240 points wide.
const WIDTH: f32 = 240.0;

const ENGLISH: &str = "Welcome, traveller. The road ahead is long, and the night is cold.";
const SPANISH: &str =
    "Bienvenido, viajero. El camino que tienes por delante es largo, y la noche es fría.";

fn write_tables(root: &Path) {
    let dir = root.join("locale");
    std::fs::create_dir_all(&dir).expect("the locale folder");
    std::fs::write(
        dir.join("en.ron"),
        format!(
            r#"(version: 1, strings: {{"dialogue.intro": "{ENGLISH}", "dialogue.name": "Old woman", "button.spanish": "Español"}})"#
        ),
    )
    .expect("English");
    std::fs::write(
        dir.join("es.ron"),
        format!(r#"(version: 1, strings: {{"dialogue.intro": "{SPANISH}", "button.spanish": "Español"}})"#),
    )
    .expect("Spanish");
}

fn spawn(world: &mut World, name: &str, rect: UiRect) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, Name::new(name));
    world.insert(e, rect);
    e
}

fn keyed(key: &str, size: f32) -> Text {
    Text {
        key: Some(key.to_owned()),
        size,
        // Wrapped to its rect: the rect is the narrower.
        max_width: Some(10_000.0),
        line_spacing: 1.2,
        ..Text::new("")
    }
}

/// The box at the bottom of the screen, its speaker's name and its line,
/// and a button that asks for Spanish.
fn dialogue(world: &mut World) -> (Entity, Entity, Entity) {
    let panel = spawn(
        world,
        "Dialogue",
        UiRect::pinned(
            Vec2::new(0.5, 1.0),
            Vec2::new(0.0, -16.0),
            Vec2::new(WIDTH + 32.0, 160.0),
        ),
    );
    world.insert(panel, Sprite2D::new([0.05, 0.05, 0.08, 0.9]));
    let speaker = spawn(
        world,
        "Speaker",
        UiRect::pinned(Vec2::ZERO, Vec2::splat(16.0), Vec2::new(WIDTH, 24.0)),
    );
    world.insert(speaker, keyed("dialogue.name", 16.0));
    hierarchy::set_parent(world, speaker, panel).expect("parents");
    let line = spawn(
        world,
        "Line",
        UiRect::pinned(Vec2::ZERO, Vec2::new(16.0, 44.0), Vec2::new(WIDTH, 100.0)),
    );
    world.insert(line, keyed("dialogue.intro", 18.0));
    hierarchy::set_parent(world, line, panel).expect("parents");

    let button = spawn(
        world,
        "Spanish",
        UiRect::pinned(Vec2::ZERO, Vec2::splat(16.0), Vec2::new(120.0, 36.0)),
    );
    world.insert(button, Sprite2D::default());
    world.insert(button, UiButton::default());
    world.insert(button, keyed("button.spanish", 16.0));
    (speaker, line, button)
}

/// The line's lines, laid out as the screen draws them: wrapped at its
/// rect's width.
fn lines(session: &Headless, line: Entity) -> Vec<f32> {
    let world = session.world();
    let text = world.get::<Text>(line).expect("the line");
    let rect = UiLayout::from_world(world, SCREEN)
        .rect(line)
        .expect("laid out");
    let width = text.max_width.map(|w| w.min(rect.size().x));
    let px = ui_px(text, 1.0);
    let fonts = session.fonts().expect("fonts");
    let layout = voltra_scene::text::shape::layout_at(text, fonts, px, width).expect("a face");
    let per_px = text.size / px as f32;
    layout.line_widths.iter().map(|w| w * per_px).collect()
}

#[test]
fn the_dialogue_wraps_and_switches_language_while_it_runs() {
    let root = scratch_root();
    copy_bundled_font(&root, DEFAULT_FONT);
    write_tables(&root);

    // The game: the button switches the language.
    let mut spanish_button: Option<Entity> = None;
    let mut app = App::default()
        .with_asset_root(&root)
        .with_language("en")
        .with_update(move |tick| {
            let button = *spanish_button.get_or_insert_with(|| {
                tick.world
                    .query::<Name>()
                    .find(|(_, n)| n.as_str() == "Spanish")
                    .map(|(e, _)| e)
                    .expect("the button")
            });
            if tick.ui.clicked(button) {
                tick.locale.set_language("es");
            }
        });
    let (speaker, line, button) = dialogue(&mut app.world);
    let mut session = Headless::new(app).with_screen(SCREEN);
    session.step(1, FRAME);

    let content = |s: &Headless, e: Entity| s.world().get::<Text>(e).expect("text").content.clone();
    assert_eq!(content(&session, line), ENGLISH);
    assert_eq!(content(&session, speaker), "Old woman");
    let english = lines(&session, line);
    assert!(english.len() >= 3, "wrapped: {english:?}");
    assert!(
        english.iter().all(|w| *w <= WIDTH + 0.01),
        "within the box: {english:?}"
    );

    // The player clicks Español; the game's tick switches; the box follows.
    let rect = UiLayout::from_world(session.world(), SCREEN)
        .rect(button)
        .expect("laid out");
    session.input_mut().on_cursor_moved(rect.center());
    session.input_mut().on_mouse_button(MouseButton::Left, true);
    session.step(1, FRAME);
    session
        .input_mut()
        .on_mouse_button(MouseButton::Left, false);
    session.step(1, FRAME);
    assert_eq!(session.locale().language(), "es");
    assert_eq!(content(&session, line), SPANISH);
    let spanish = lines(&session, line);
    assert!(spanish.len() >= 3, "wrapped: {spanish:?}");
    assert_ne!(spanish, english, "laid out again for the new words");
    assert!(spanish.iter().all(|w| *w <= WIDTH + 0.01));
    // Spanish has no speaker name: the key shows where it is missing.
    assert_eq!(content(&session, speaker), "dialogue.name");

    // And back, from code.
    assert!(session.set_language("en"));
    session.step(1, FRAME);
    assert_eq!(content(&session, line), ENGLISH);
    assert_eq!(content(&session, speaker), "Old woman");
}

#[test]
fn a_language_with_no_table_shows_the_keys() {
    let root = scratch_root();
    copy_bundled_font(&root, DEFAULT_FONT);
    write_tables(&root);
    let mut app = App::default().with_asset_root(&root).with_language("fr");
    let (_, line, _) = dialogue(&mut app.world);
    let mut session = Headless::new(app);
    session.step(1, FRAME);
    assert_eq!(
        session.world().get::<Text>(line).expect("text").content,
        "dialogue.intro"
    );
}
