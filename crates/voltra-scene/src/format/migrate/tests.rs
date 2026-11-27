use super::*;
use crate::format::SceneError;
use crate::{Camera2D, Collider2D, RigidBody2D, Sprite2D, Transform2D};

/// A version 1 file holding `components` (name, RON) on one entity.
fn v1(components: &[(&str, &str)]) -> String {
    let fields: String = components
        .iter()
        .map(|(name, value)| format!("\"{name}\": {value},"))
        .collect();
    format!(
        "(version: 1, entities: [(id: \"019fe300-0000-7000-8000-000000000001\", \
         components: {{{fields}}})])"
    )
}

#[test]
fn every_rename_reaches_a_registered_name() {
    // The table's last row lands on what this build registers: a rename
    // whose target nobody registers would read every old file as unknown.
    let registry = crate::format::ComponentRegistry::with_defaults();
    let names: Vec<&str> = registry.names().collect();
    for (_, renames) in RENAMES {
        for (old, new) in *renames {
            assert!(names.contains(new), "{old} -> {new} is not registered");
            assert!(!names.contains(old), "{old} is still registered");
        }
    }
    assert_eq!(
        [
            Transform2D::NAME,
            Camera2D::NAME,
            Sprite2D::NAME,
            RigidBody2D::NAME,
            Collider2D::NAME
        ],
        [
            "Transform2D",
            "Camera2D",
            "Sprite2D",
            "RigidBody2D",
            "Collider2D"
        ]
    );
}

#[test]
fn a_table_row_for_every_older_version() {
    for version in OLDEST..VERSION {
        assert!(renames_from(version).is_some(), "no row from {version}");
    }
    assert!(
        renames_from(VERSION).is_none(),
        "no row past the current one"
    );
}

#[test]
fn a_version_one_file_is_renamed_as_it_is_read() {
    let text = v1(&[
        ("Transform", "(translation: (1.0, 2.0))"),
        ("Sprite", "()"),
        ("Name", "\"crate\""),
    ]);
    let file = parse(&text).expect("a version 1 file reads");
    assert_eq!(file.version, VERSION);
    let keys: Vec<&str> = file.entities[0]
        .components
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["Name", "Sprite2D", "Transform2D"]);
    // The values are the file's own text, untouched.
    assert_eq!(
        file.entities[0].components["Transform2D"].get_ron(),
        "(translation: (1.0, 2.0))"
    );
}

#[test]
fn an_unknown_component_survives_migration() {
    let text = v1(&[("Health", "(hp: 3)"), ("Collider", "Circle(radius: 0.5)")]);
    let file = parse(&text).expect("reads");
    let components = &file.entities[0].components;
    assert_eq!(components["Health"].get_ron(), "(hp: 3)");
    assert!(components.contains_key("Collider2D"));
    assert!(!components.contains_key("Collider"));
}

#[test]
fn a_part_s_overrides_are_renamed_with_the_components() {
    let text = v1(&[(
        "PrefabPart",
        "(record: \"019fe300-0000-7000-8000-0000000000aa\", \
         overridden: [\"Camera\", \"Health\", \"RigidBody\", \"Transform\"])",
    )]);
    let file = parse(&text).expect("reads");
    let part: PrefabPart = file.entities[0].components[PrefabPart::NAME]
        .into_rust()
        .expect("still a part");
    let overridden: Vec<&str> = part.overridden.iter().map(String::as_str).collect();
    // A name that did not move — a game's own — is left as it was.
    assert_eq!(
        overridden,
        ["Camera2D", "Health", "RigidBody2D", "Transform2D"]
    );
}

#[test]
fn a_part_overriding_nothing_renamed_keeps_its_text() {
    let raw = "(record: \"019fe300-0000-7000-8000-0000000000aa\", overridden: [\"Name\"])";
    let file = parse(&v1(&[("PrefabPart", raw)])).expect("reads");
    assert_eq!(file.entities[0].components["PrefabPart"].get_ron(), raw);
}

#[test]
fn a_current_file_comes_back_as_it_was() {
    let text = v1(&[("Transform", "()")]).replace("version: 1", "version: 2");
    let file = parse(&text).expect("reads");
    // Not renamed: version 2 has no `Transform`, so this one is unknown.
    assert!(file.entities[0].components.contains_key("Transform"));
    let borrowed = current(&file).expect("current");
    assert!(matches!(borrowed, Cow::Borrowed(_)));
}

#[test]
fn a_newer_or_a_zero_version_is_refused() {
    for version in [0, VERSION + 1, 99] {
        let text = v1(&[]).replace("version: 1", &format!("version: {version}"));
        match parse(&text) {
            Err(SceneError::UnsupportedVersion { found, supported }) => {
                assert_eq!((found, supported), (version, VERSION));
            }
            other => panic!("version {version} read: {other:?}"),
        }
    }
}

#[test]
fn an_entity_holding_both_names_is_refused() {
    let text = v1(&[("Sprite", "()"), ("Sprite2D", "(sides: 3)")]);
    match parse(&text) {
        Err(SceneError::Component { name, .. }) => assert_eq!(name, "Sprite2D"),
        other => panic!("both names read: {other:?}"),
    }
}

#[test]
fn the_version_written_is_reported() {
    let (file, written) = parse_versioned(&v1(&[])).expect("reads");
    assert_eq!((file.version, written), (VERSION, 1));
}
