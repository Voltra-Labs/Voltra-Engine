use std::path::Path;

use voltra_ecs::World;
use voltra_testkit::scratch_root;

use super::*;
use crate::Text;

fn write(root: &Path, language: &str, body: &str) {
    let dir = root.join(LOCALE_DIR);
    std::fs::create_dir_all(&dir).expect("the locale folder");
    std::fs::write(
        dir.join(format!("{language}.ron")),
        format!("(version: 1, strings: {{{body}}})"),
    )
    .expect("the table");
}

fn tables() -> voltra_testkit::Scratch {
    let root = scratch_root();
    write(&root, "en", r#""greet": "Hello", "bye": "Goodbye""#);
    write(&root, "es", r#""greet": "Hola""#);
    root
}

#[test]
fn a_version_2_table_names_its_fonts_and_a_version_1_names_none() {
    let root = tables();
    std::fs::write(
        root.join(LOCALE_DIR).join("ar.ron"),
        r#"(version: 2, fonts: ["fonts/NotoSansArabic-Regular.ttf"], strings: {"greet": "مرحبا"})"#,
    )
    .expect("the table");
    let mut locale = Locale::open(&root, "en");
    assert!(locale.fonts().is_empty(), "version 1: no fonts");
    assert!(locale.set_language("ar"));
    assert_eq!(locale.text("greet"), "مرحبا");
    assert_eq!(
        locale.fonts(),
        [AssetPath::new("fonts/NotoSansArabic-Regular.ttf").expect("a path")]
    );
    assert!(locale.set_language("en"));
    assert!(
        locale.fonts().is_empty(),
        "a switch drops the last table's fonts"
    );
}

#[test]
fn applied_fonts_become_the_fallback_chain() {
    let root = tables();
    std::fs::write(
        root.join(LOCALE_DIR).join("ar.ron"),
        r#"(version: 2, fonts: ["fonts/NotoSansArabic-Regular.ttf"])"#,
    )
    .expect("the table");
    voltra_testkit::copy_workspace_asset(&root, voltra_testkit::NOTO_ARABIC);
    let mut fonts = Fonts::new(&root);
    let locale = Locale::open(&root, "ar");
    locale.apply_fonts(&mut fonts);
    assert_eq!(fonts.fallbacks().len(), 1);
    let arabic = fonts
        .by_path_handle(&AssetPath::new(voltra_testkit::NOTO_ARABIC).expect("a path"))
        .expect("loaded");
    assert_eq!(
        fonts.chain(fonts.default_font()),
        vec![fonts.default_font(), arabic]
    );
}

#[test]
fn a_key_reads_in_the_current_language() {
    let root = tables();
    let mut locale = Locale::open(&root, "en");
    assert_eq!(locale.text("greet"), "Hello");
    assert!(locale.set_language("es"));
    assert_eq!(locale.language(), "es");
    assert_eq!(locale.text("greet"), "Hola");
}

#[test]
fn a_missing_string_shows_its_key() {
    let root = tables();
    let locale = Locale::open(&root, "es");
    assert_eq!(locale.text("bye"), "bye");
    assert_eq!(locale.text("bye"), "bye", "and again, warned once");
    assert!(locale.missing.borrow().contains("bye"));
}

#[test]
fn a_language_with_no_table_shows_keys_and_says_so() {
    let root = tables();
    let mut locale = Locale::open(&root, "en");
    assert!(!locale.set_language("fr"));
    assert_eq!(locale.language(), "fr");
    assert_eq!(locale.text("greet"), "greet");
    assert!(!locale.set_language("../etc"), "no file has that name");
}

#[test]
fn keyed_texts_follow_the_language_and_plain_ones_stay() {
    let root = tables();
    let mut locale = Locale::open(&root, "en");
    let mut world = World::new();
    let keyed = world.spawn();
    world.insert(
        keyed,
        Text {
            key: Some("greet".to_owned()),
            ..Text::new("")
        },
    );
    let plain = world.spawn();
    world.insert(plain, Text::new("Score"));
    locale.localize(&mut world);
    assert_eq!(world.get::<Text>(keyed).expect("text").content, "Hello");
    locale.set_language("es");
    locale.localize(&mut world);
    assert_eq!(world.get::<Text>(keyed).expect("text").content, "Hola");
    assert_eq!(world.get::<Text>(plain).expect("text").content, "Score");
}

#[test]
fn a_saved_table_is_read_again() {
    let root = tables();
    let mut locale = Locale::open(&root, "es");
    write(&root, "es", r#""greet": "Buenas""#);
    let other = AssetPath::new("locale/en.ron").expect("a path");
    assert!(!locale.reload(&other), "not the current table");
    let path = table_path("es").expect("a path");
    assert!(locale.reload(&path));
    assert_eq!(locale.text("greet"), "Buenas");
}

#[test]
fn a_newer_table_is_refused() {
    assert!(matches!(
        StringTable::parse("(version: 9, strings: {})"),
        Err(TableError::Version(9))
    ));
}
