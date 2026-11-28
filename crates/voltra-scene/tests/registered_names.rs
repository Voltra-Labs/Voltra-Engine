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
    // Rust type names a new game may not take, the template's among them.
    (
        "crates/xtask/src/new_game.rs",
        r#""Game" | "Tick" | "Player" | "App""#,
    ),
    // The template's test finds the player by its entity name.
    (
        "crates/xtask/template/plugin.rs.tpl",
        r#"name.as_str() == "Player""#,
    ),
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

/// The names `files`' own `NAME` consts give: a game's components, which
/// only that game's code (or the template's) may be held to — a "Player"
/// elsewhere is a lobby row, not the game's component.
fn own_names(files: &[PathBuf]) -> Vec<String> {
    let mut names = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(file).expect("a source file reads");
        for line in text.lines() {
            let quoted = line
                .trim()
                .strip_prefix("pub const NAME: &'static str = \"")
                .and_then(|rest| rest.strip_suffix("\";"));
            if let Some(name) = quoted {
                names.push(name.to_owned());
            }
        }
    }
    names
}

/// The lines of the migration table: history, each name as its version
/// spelled it.
fn in_renames_table(lines: &[&str], number: usize) -> bool {
    let Some(start) = lines
        .iter()
        .position(|l| l.starts_with("pub const RENAMES"))
    else {
        return false;
    };
    let end = lines[start..]
        .iter()
        .position(|l| l.starts_with(")];"))
        .map_or(lines.len(), |at| start + at);
    (start..=end).contains(&number)
}

#[test]
fn no_code_spells_a_registered_name() {
    let root = workspace();
    let engine: Vec<String> = ComponentRegistry::with_defaults()
        .names()
        .map(str::to_owned)
        .collect();
    assert!(engine.len() > 40, "the registry lists its names");

    // Each member's sources, with the names they are held to: the
    // engine's everywhere, and a game's (or the template's) own in it.
    let mut members: Vec<(Vec<PathBuf>, Vec<String>)> = Vec::new();
    for top in ["crates", "games"] {
        let Ok(dirs) = std::fs::read_dir(root.join(top)) else {
            continue;
        };
        for member in dirs.flatten() {
            let mut files = Vec::new();
            for dir in ["src", "template", "examples", "benches"] {
                sources(&member.path().join(dir), &mut files);
            }
            let mut names = engine.clone();
            if top == "games" || member.file_name() == "xtask" {
                let own = own_names(&files);
                assert!(
                    top != "games" || !own.is_empty(),
                    "{:?} registers no NAME of its own",
                    member.path()
                );
                names.extend(own);
            }
            members.push((files, names));
        }
    }
    let count: usize = members.iter().map(|(files, _)| files.len()).sum();
    assert!(count > 500, "found the sources ({count})");

    let mut found = Vec::new();
    for (file, names) in members
        .iter()
        .flat_map(|(files, names)| files.iter().map(move |file| (file, names)))
    {
        let text = std::fs::read_to_string(file).expect("a source file reads");
        let relative = file
            .strip_prefix(&root)
            .expect("under the root")
            .to_string_lossy()
            .replace('\\', "/");
        let lines: Vec<&str> = text.lines().collect();
        // Inside a `#[cfg(test)]` module, by brace depth: zero is outside.
        let mut in_tests = 0usize;
        let mut module_line = false;
        for (number, line) in lines.iter().enumerate() {
            if std::mem::take(&mut module_line) {
                continue;
            }
            if in_tests > 0 {
                in_tests = (in_tests + line.matches('{').count())
                    .saturating_sub(line.matches('}').count());
                continue;
            }
            let test_module = line.trim() == "#[cfg(test)]"
                && lines
                    .get(number + 1)
                    .is_some_and(|next| next.trim_start().starts_with("mod "));
            if test_module {
                let next = lines[number + 1];
                // `mod tests;` is another file, left out by its name.
                in_tests = next.matches('{').count();
                module_line = true;
                continue;
            }
            let code = line.trim_start();
            if code.starts_with("//") || code.contains("const NAME: &'static str") {
                continue;
            }
            if relative == "crates/voltra-scene/src/format/migrate.rs"
                && in_renames_table(&lines, number)
            {
                continue;
            }
            let allowed = NOT_THE_COMPONENT
                .iter()
                .any(|(path, text)| relative == *path && line.contains(text));
            if allowed {
                continue;
            }
            for name in names {
                // Quoted, or quoted inside a string (`"{\"Sprite2D\": …}"`).
                if line.contains(&format!("\"{name}\""))
                    || line.contains(&format!("\\\"{name}\\\""))
                {
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
