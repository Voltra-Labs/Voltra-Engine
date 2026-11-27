//! `cargo xtask new-game <name>`: a game crate in this workspace's
//! `games/`, ready to play, to open in the editor, and to package.
//!
//! In this workspace, not a project of its own: a game and the editor that
//! plays it must be built from one workspace — cargo hashes a path
//! dependency by its path from the workspace's root, and the engine's
//! types with it — or the editor's stamp refuses the game's library
//! (`voltra_core::game::stamp`). `games/*` is a member glob, so the new
//! crate builds at once.

use std::path::{Path, PathBuf};

/// The template's text files, and where each goes in the new crate.
const TEMPLATE: [(&str, &str); 8] = [
    ("Cargo.toml", include_str!("../template/Cargo.toml.tpl")),
    ("src/lib.rs", include_str!("../template/lib.rs.tpl")),
    ("src/main.rs", include_str!("../template/main.rs.tpl")),
    ("tests/plugin.rs", include_str!("../template/plugin.rs.tpl")),
    ("tests/lobby.rs", include_str!("../template/lobby.rs.tpl")),
    ("build.ron", include_str!("../template/build.ron.tpl")),
    ("README.md", include_str!("../template/README.md.tpl")),
    (
        "assets/scenes/main.ron",
        include_str!("../template/main.ron.tpl"),
    ),
];

/// The engine's own assets a game starts with: the input map, sorting
/// layers and audio buses the loop reads by convention, the default font and
/// its licence, a texture for the player, and the options and lobby
/// screens (`voltra_core::options`, `voltra_core::lobby`) to open from a
/// menu.
const COPIED: [&str; 8] = [
    "default.input.ron",
    "default.layers.ron",
    "default.buses.ron",
    "fonts/Ubuntu-Light.ttf",
    "fonts/UFL.txt",
    "sprites/checker.png",
    "prefabs/options.prefab.ron",
    "prefabs/lobby.prefab.ron",
];

/// What a new game is called, three ways.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Names {
    /// The package: `my-game`.
    pub package: String,
    /// The crate as Rust names it: `my_game`.
    pub krate: String,
    /// The game's type: `MyGame`.
    pub type_name: String,
    /// What the window and the build are called: `My Game`, or as given.
    pub title: String,
}

impl Names {
    /// Checks `package` is a name cargo and Rust both take, and derives the
    /// rest from it.
    pub fn new(package: &str, title: Option<&str>) -> Result<Self, String> {
        let starts_well = package
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase());
        let well_formed = package
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !starts_well || !well_formed || package.ends_with('-') || package.contains("--") {
            return Err(format!(
                "`{package}` is not a game name: lowercase letters, digits and single dashes, starting with a letter"
            ));
        }
        let words: Vec<&str> = package.split('-').collect();
        let capital = |word: &str| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        };
        let type_name: String = words.iter().map(|w| capital(w)).collect();
        // The game's type sits beside `Game` and `Tick`; a game called
        // `game` would shadow the trait.
        if matches!(type_name.as_str(), "Game" | "Tick" | "Player" | "App") {
            return Err(format!(
                "`{package}` names a type the template already uses"
            ));
        }
        Ok(Self {
            krate: package.replace('-', "_"),
            title: title.map_or_else(
                || {
                    words
                        .iter()
                        .map(|w| capital(w))
                        .collect::<Vec<_>>()
                        .join(" ")
                },
                str::to_owned,
            ),
            package: package.to_owned(),
            type_name,
        })
    }

    fn fill(&self, template: &str) -> String {
        template
            .replace("{{package}}", &self.package)
            .replace("{{crate}}", &self.krate)
            .replace("{{type}}", &self.type_name)
            .replace("{{title}}", &escape(&self.title))
    }
}

/// `title` as it can stand inside a Rust and a RON string literal.
fn escape(title: &str) -> String {
    title.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Writes the game `names` describes into `workspace/games/<package>`, with
/// the engine's starting assets copied from `workspace/assets`. Refuses a
/// folder that is there already, rather than writing over a game.
pub fn create(workspace: &Path, names: &Names) -> Result<PathBuf, String> {
    let dir = workspace.join("games").join(&names.package);
    if dir.exists() {
        return Err(format!("{} is there already", dir.display()));
    }
    for (path, template) in TEMPLATE {
        write(&dir.join(path), &names.fill(template))?;
    }
    let assets = workspace.join("assets");
    for path in COPIED {
        let to = dir.join("assets").join(path);
        create_parent(&to)?;
        std::fs::copy(assets.join(path), &to)
            .map_err(|e| format!("cannot copy {path} from {}: {e}", assets.display()))?;
    }
    Ok(dir)
}

fn create_parent(path: &Path) -> Result<(), String> {
    match path.parent() {
        Some(parent) => std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display())),
        None => Ok(()),
    }
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    create_parent(path)?;
    std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// What to do next, for the terminal.
pub fn next_steps(names: &Names) -> String {
    let package = &names.package;
    format!(
        "made games/{package}\n\n  cargo run -p {package}\n  cargo build -p voltra-editor -p {package}\n  cargo run -p voltra-editor -- --game games/{package}/build.ron scenes/main.ron"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "voltra-new-game-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        dir
    }

    fn workspace() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn a_name_becomes_a_package_a_crate_a_type_and_a_title() {
        let names = Names::new("space-trader-2", None).expect("a good name");
        assert_eq!(names.krate, "space_trader_2");
        assert_eq!(names.type_name, "SpaceTrader2");
        assert_eq!(names.title, "Space Trader 2");
        let titled = Names::new("st", Some("Space \"Trader\"")).expect("titled");
        assert_eq!(titled.title, "Space \"Trader\"");
        assert_eq!(escape(&titled.title), "Space \\\"Trader\\\"");
    }

    #[test]
    fn names_cargo_or_the_template_would_refuse_are_refused() {
        for bad in [
            "",
            "My-Game",
            "2fast",
            "a--b",
            "trailing-",
            "with space",
            "game",
            "tick",
        ] {
            assert!(Names::new(bad, None).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_new_game_has_every_file_filled_in_and_packages_as_it_is() {
        let root = scratch();
        let assets = root.join("assets");
        for path in COPIED {
            let from = workspace().join("assets").join(path);
            let to = assets.join(path);
            std::fs::create_dir_all(to.parent().expect("a folder")).expect("dir");
            std::fs::copy(from, to).expect("the engine's asset");
        }
        let names = Names::new("my-game", None).expect("a name");

        let dir = create(&root, &names).expect("made");

        for (path, _) in TEMPLATE {
            let text = std::fs::read_to_string(dir.join(path)).expect("written");
            assert!(
                !text.contains("{{"),
                "{path} still has a placeholder:\n{text}"
            );
        }
        for path in COPIED {
            assert!(dir.join("assets").join(path).is_file(), "{path}");
        }
        let lib = std::fs::read_to_string(dir.join("src/lib.rs")).expect("lib");
        assert!(lib.contains("pub struct MyGame;") && lib.contains("export_game!(MyGame)"));

        // The manifest is a build `dist` and `dist --web` plan as they are.
        let plan = crate::dist::plan(&dir.join("build.ron"), &root.join("dist")).expect("plans");
        assert_eq!(plan.manifest.name, "My Game");
        let web = crate::web::plan(&plan, &root.join("dist"));
        assert_eq!(web.cargo_args, ["--package", "my-game"]);

        assert!(create(&root, &names).is_err(), "never over a game");
    }
}
