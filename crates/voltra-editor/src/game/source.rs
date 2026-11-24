//! Where a game's sources are and how to build them, from cargo itself.

use std::path::{Path, PathBuf};
use std::process::Command;

use voltra_core::game::builder::cargo;

/// What rebuilding a game takes: the folder to watch and the build to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GameSource {
    /// The game crate's own folder.
    pub dir: PathBuf,
    /// Where the build runs: the workspace `build.ron` is in.
    pub workspace: PathBuf,
    /// `cargo` arguments that build the game's library.
    pub build: Vec<String>,
}

/// The source of `package` in the workspace at `project`, asking
/// `cargo metadata` where it lives.
pub(crate) fn locate(project: &Path, package: &str) -> Result<GameSource, String> {
    let output = Command::new(cargo())
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(project)
        .output()
        .map_err(|e| format!("cannot run cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("cannot read cargo metadata: {e}"))?;
    from_metadata(&metadata, package, project)
}

/// [`locate`], from what `cargo metadata` answered.
fn from_metadata(
    metadata: &serde_json::Value,
    package: &str,
    project: &Path,
) -> Result<GameSource, String> {
    let packages = metadata["packages"]
        .as_array()
        .ok_or("cargo metadata lists no packages")?;
    let manifest = |name: &str| {
        packages
            .iter()
            .find(|p| p["name"].as_str() == Some(name))
            .and_then(|p| p["manifest_path"].as_str())
            .map(PathBuf::from)
    };
    let game =
        manifest(package).ok_or_else(|| format!("no package {package} in this workspace"))?;
    let dir = game
        .parent()
        .map(Path::to_path_buf)
        .ok_or("the game's manifest is in no folder")?;
    // Built with the editor when the editor is in the same workspace, so
    // every dependency resolves the features the running editor was built
    // with — or the stamp would refuse the result.
    let editor = env!("CARGO_PKG_NAME");
    let mut build = vec![
        "build".to_owned(),
        "--package".to_owned(),
        package.to_owned(),
    ];
    if manifest(editor).is_some() {
        build.extend(["--package".to_owned(), editor.to_owned()]);
    }
    if !cfg!(debug_assertions) {
        build.push("--release".to_owned());
    }
    Ok(GameSource {
        dir,
        workspace: project.to_path_buf(),
        build,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_is_built_with_the_editor_when_they_share_a_workspace() {
        let metadata = serde_json::json!({
            "packages": [
                {"name": "my-game", "manifest_path": "/w/games/my-game/Cargo.toml"},
                {"name": "voltra-editor", "manifest_path": "/w/crates/voltra-editor/Cargo.toml"},
            ]
        });
        let source = from_metadata(&metadata, "my-game", Path::new("/w")).expect("found");

        assert_eq!(source.dir, PathBuf::from("/w/games/my-game"));
        assert!(source
            .build
            .windows(2)
            .any(|w| w == ["--package", "voltra-editor"]));
        assert!(source
            .build
            .windows(2)
            .any(|w| w == ["--package", "my-game"]));
    }

    #[test]
    fn a_game_alone_in_its_workspace_is_built_alone_and_a_missing_one_said() {
        let metadata = serde_json::json!({
            "packages": [{"name": "my-game", "manifest_path": "/g/Cargo.toml"}]
        });
        let source = from_metadata(&metadata, "my-game", Path::new("/g")).expect("found");
        assert!(!source.build.iter().any(|a| a == "voltra-editor"));
        assert!(from_metadata(&metadata, "other", Path::new("/g")).is_err());
    }

    #[test]
    fn this_workspace_knows_where_the_platformer_is() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source = locate(&root, "voltra-platformer").expect("cargo metadata answers");
        assert!(source.dir.ends_with("games/platformer"), "{source:?}");
    }
}
