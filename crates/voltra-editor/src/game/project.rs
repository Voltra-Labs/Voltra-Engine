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
    /// The asset root, relative to the manifest's folder.
    #[serde(default = "default_assets")]
    assets: PathBuf,
}

fn default_assets() -> PathBuf {
    PathBuf::from("assets")
}

/// The asset root the build manifest at `manifest` names: what the editor
/// opens a game's scenes from when started with `--game` that manifest.
pub(crate) fn assets_of(manifest: &Path) -> Result<PathBuf, String> {
    let text = std::fs::read_to_string(manifest)
        .map_err(|e| format!("cannot read {}: {e}", manifest.display()))?;
    let parsed = read(&text).map_err(|e| format!("cannot read {}: {e}", manifest.display()))?;
    let dir = manifest
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Ok(dir.join(parsed.assets))
}

#[derive(Debug, serde::Deserialize)]
struct GameCrate {
    package: String,
}

/// A game the project names: its crate, the folder of the manifest naming
/// it, and its library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NamedGame {
    pub package: String,
    /// Where `build.ron` is: the workspace the crate is built in.
    pub project: PathBuf,
    pub library: PathBuf,
}

/// The game the project names, if it names one: `build.ron` beside the
/// asset root says which crate, and the library is looked for beside this
/// editor's executable — where `cargo build -p voltra-editor -p <game>`
/// puts both, built together as they must be (`voltra_core::game::stamp`).
pub(crate) fn named(asset_root: &Path) -> Result<Option<NamedGame>, String> {
    let Some(manifest) = asset_root.parent().map(|dir| dir.join(MANIFEST)) else {
        return Ok(None);
    };
    named_by(&manifest)
}

/// The game the build manifest at `manifest` names; `None` when it names
/// none.
pub(crate) fn named_by(manifest: &Path) -> Result<Option<NamedGame>, String> {
    let Some(package) = game_package(manifest)? else {
        return Ok(None);
    };
    let exe = std::env::current_exe().map_err(|e| format!("cannot find this editor: {e}"))?;
    let dir = exe.parent().ok_or("this editor is in no folder")?;
    // `platformer.build.ron` has an empty parent: the current folder.
    let project = manifest
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    Ok(Some(NamedGame {
        library: dir.join(library_file(&package)),
        package,
        project,
    }))
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
    Ok(read(text)?.game.map(|game| game.package))
}

fn read(text: &str) -> Result<Manifest, ron::error::SpannedError> {
    // `game: (package: …)` with `Some` implied, as the player reads it.
    let options =
        ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    options.from_str(text)
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
    fn the_asset_root_is_beside_the_manifest() {
        let dir = std::env::temp_dir().join(format!("voltra-assets-of-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let manifest = dir.join("build.ron");
        std::fs::write(&manifest, r#"(version: 1, name: "G", assets: "content")"#).expect("write");
        assert_eq!(assets_of(&manifest), Ok(dir.join("content")));
        std::fs::write(&manifest, r#"(version: 1, name: "G")"#).expect("write");
        assert_eq!(assets_of(&manifest), Ok(dir.join("assets")));
    }

    #[test]
    fn a_project_with_no_game_has_none() {
        let text = r#"(version: 1, name: "Sandbox", scene: "scenes/sandbox.ron")"#;
        assert_eq!(parse(text).expect("parses"), None);
        assert_eq!(game_package(Path::new("no/such/build.ron")), Ok(None));
    }

    #[test]
    fn a_manifest_named_without_a_folder_is_in_the_current_one() {
        let named = named_by(Path::new("platformer.build.ron"));
        // Run from the crate's folder, where there is no such file.
        assert_eq!(named, Ok(None));
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../platformer.build.ron");
        let named = named_by(&root).expect("reads").expect("names a game");
        assert!(!named.project.as_os_str().is_empty());
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
