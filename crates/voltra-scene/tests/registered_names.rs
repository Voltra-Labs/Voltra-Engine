//! A registered component name is written once, by its type's `NAME`
//! const: no code outside that const spells the string, so renaming a
//! component is one line here and one row of the migration table.
//!
//! Scans every crate's and game's sources (tests excepted: they pin what a
//! file says, and a fixture spelling the old name is the point of one).

use std::path::{Path, PathBuf};

use voltra_scene::ComponentRegistry;

/// Quoted names that are not the component: an entity's default name, a
/// text's default content, a label. `(file, the text on the line)`.
const NOT_THE_COMPONENT: &[(&str, &str)] = &[
    // A new text's placeholder content.
    ("crates/voltra-scene/src/text.rs", r#"content: "Text""#),
    // A field label in the atlas editor.
    (
        "crates/voltra-editor/src/panels/atlas_editor.rs",
        r#"ui.label("Name")"#,
    ),
    // The lobby screen's part called "Name" (the name field).
    (
        "crates/voltra-core/src/lobby/screen.rs",
        r#"parts.get("Name")"#,
    ),
    // New entities' default names in the editor.
    (
        "crates/voltra-editor/src/panels/menu_bar/spawn_items.rs",
        r#"spawn::sprite(frame, "Sprite""#,
    ),
    (
        "crates/voltra-editor/src/panels/menu_bar/spawn_items.rs",
        r#"spawn::camera(frame, "Camera""#,
    ),
    (
        "crates/voltra-editor/src/panels/menu_bar/spawn_items.rs",
        r#"spawn::tilemap(frame, "Tilemap""#,
    ),
    (
        "crates/voltra-editor/src/panels/menu_bar/spawn_items.rs",
        r#"spawn::text(frame, "Text""#,
    ),
    (
        "crates/voltra-editor/src/panels/viewport/drop.rs",
        r#"spawn::text(frame, "Text""#,
    ),
];

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

/// Every `.rs` (and template) file under `dir`, test files left out.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "tests" && name != "target" {
                sources(&path, out);
            }
        } else if (name.ends_with(".rs") || name.ends_with(".rs.tpl"))
            && name != "tests.rs"
            && !name.ends_with("_tests.rs")
        {
            out.push(path);
        }
    }
}

#[test]
fn no_code_spells_a_registered_name() {
    let root = workspace();
    let names: Vec<&str> = ComponentRegistry::with_defaults().names().collect();
    assert!(names.len() > 40, "the registry lists its names");

    let mut files = Vec::new();
    for top in ["crates", "games"] {
        let Ok(members) = std::fs::read_dir(root.join(top)) else {
            continue;
        };
        for member in members.flatten() {
            sources(&member.path().join("src"), &mut files);
            sources(&member.path().join("template"), &mut files);
        }
    }
    assert!(files.len() > 500, "found the sources ({})", files.len());

    let mut found = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("a source file reads");
        let relative = file
            .strip_prefix(&root)
            .expect("under the root")
            .to_string_lossy()
            .replace('\\', "/");
        for (number, line) in text.lines().enumerate() {
            // A file's own unit tests are its last module.
            if line.trim() == "#[cfg(test)]" {
                break;
            }
            let code = line.trim_start();
            if code.starts_with("//") || code.contains("const NAME: &'static str") {
                continue;
            }
            let allowed = NOT_THE_COMPONENT
                .iter()
                .any(|(path, text)| relative == *path && line.contains(text));
            if allowed {
                continue;
            }
            for name in &names {
                if line.contains(&format!("\"{name}\"")) {
                    found.push(format!("{relative}:{}: \"{name}\"", number + 1));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "registered names spelled outside their NAME const (use `Type::NAME`):\n{}",
        found.join("\n")
    );
}
