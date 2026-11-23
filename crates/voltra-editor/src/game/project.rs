//! Where the project's game library is: named by `build.ron`, built beside
//! this editor.

use std::path::{Path, PathBuf};

use voltra_core::game::library::library_file;

/// The file the project names its game in.
const MANIFEST: &str = "build.ron";

/// What the editor reads of `build.ron`: the game's crate, if it has one.
/// Everything else in the file is the build's business and is skipped.
#[derive(Debug, serde::Deserialize)]
struct Manifest {
    #[serde(default)]
    game: Option<GameCrate>,
}

#[derive(Debug, serde::Deserialize)]
struct GameCrate {
    package: String,
}

/// The game library the project names, if it names one: `build.ron` beside
/// the asset root says which crate, and the library is looked for beside
/// this editor's executable — where `cargo build -p voltra-editor -p <game>`
/// puts both, built together as they must be (`voltra_core::game::stamp`).
pub(crate) fn library(asset_root: &Path) -> Result<Option<PathBuf>, String> {
    let Some(manifest) = asset_root.parent().map(|dir| dir.join(MANIFEST)) else {
        return Ok(None);
    };
    library_of(&manifest)
}

/// The game library the build manifest at `manifest` names, beside this
/// editor; `None` when it names none.
pub(crate) fn library_of(manifest: &Path) -> Result<Option<PathBuf>, String> {
    let Some(package) = game_package(manifest)? else {
        return Ok(None);
    };
    let exe = std::env::current_exe().map_err(|e| format!("cannot find this editor: {e}"))?;
    let dir = exe.parent().ok_or("this editor is in no folder")?;
    Ok(Some(dir.join(library_file(&package))))
}

/// The game crate `manifest` names: `None` for a project with no game, or
/// no `build.ron` at all.
fn game_package(manifest: &Path) -> Result<Option<String>, String> {
    let text = match std::fs::read_to_string(manifest) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot read {}: {e}", manifest.display())),
    };
    parse(&text).map_err(|e| format!("cannot read {}: {e}", manifest.display()))
}

fn parse(text: &str) -> Result<Option<String>, ron::error::SpannedError> {
    // `game: (package: …)` with `Some` implied, as the player reads it.
    let options =
        ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let manifest: Manifest = options.from_str(text)?;
    Ok(manifest.game.map(|game| game.package))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_crate_is_read_and_the_rest_skipped() {
        let text = r#"(version: 1, name: "Plat", window: (width: 8, height: 8), game: (package: "my-game", bin: "my-game"))"#;
        assert_eq!(parse(text).expect("parses"), Some("my-game".to_owned()));
    }

    #[test]
    fn a_project_with_no_game_has_none() {
        let text = r#"(version: 1, name: "Sandbox", scene: "scenes/sandbox.ron")"#;
        assert_eq!(parse(text).expect("parses"), None);
        assert_eq!(game_package(Path::new("no/such/build.ron")), Ok(None));
    }

    #[test]
    fn this_repository_names_no_game_and_its_platformer_manifest_one() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        assert_eq!(game_package(&root.join("build.ron")), Ok(None));
        assert_eq!(
            game_package(&root.join("platformer.build.ron")),
            Ok(Some("voltra-platformer".to_owned()))
        );
    }
}
