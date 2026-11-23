//! What to run: the command line, filled in by a build manifest.

use std::fmt;
use std::path::{Path, PathBuf};

use voltra_player::{Manifest, ManifestError, MANIFEST_FILE};

use crate::args::Args;

/// Everything the player needs to open its window.
#[derive(Debug, PartialEq)]
pub struct Launch {
    pub scene: PathBuf,
    pub asset_root: Option<PathBuf>,
    pub title: String,
    pub size: Option<(u32, u32)>,
}

#[derive(Debug)]
pub enum LaunchError {
    /// No scene on the command line and no manifest beside the player.
    NoScene,
    /// A manifest describing a game with its own binary, which is that
    /// binary's to run, not the player's.
    GameBuild,
    Manifest(ManifestError),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoScene => write!(
                f,
                "no scene given, and no {MANIFEST_FILE} beside the player to say which"
            ),
            Self::GameBuild => write!(
                f,
                "this build names a game binary and no start scene; run the game, not the player"
            ),
            Self::Manifest(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for LaunchError {}

/// Resolves `args` against a manifest.
///
/// The manifest is `--manifest`'s, or — when no scene was named either —
/// the one beside the executable in `exe_dir`: double-clicking a build runs
/// the game. A scene on the command line with no `--manifest` is a plain
/// run, exactly as before manifests existed. Anything the command line says
/// wins over the manifest.
pub fn resolve(args: &Args, exe_dir: Option<&Path>) -> Result<Launch, LaunchError> {
    let manifest_path = match (&args.manifest, &args.scene) {
        (Some(path), _) => Some(path.clone()),
        (None, None) => exe_dir
            .map(|dir| dir.join(MANIFEST_FILE))
            .filter(|path| voltra_assets::fs::exists(path)),
        (None, Some(_)) => None,
    };
    let manifest = manifest_path
        .as_deref()
        .map(|path| {
            Manifest::load(path).map(|m| {
                let dir = path.parent().unwrap_or(Path::new("")).to_path_buf();
                (m, dir)
            })
        })
        .transpose()
        .map_err(LaunchError::Manifest)?;

    let scene = match (&args.scene, &manifest) {
        (Some(scene), _) => scene.clone(),
        (None, Some((manifest, dir))) => manifest.scene_path(dir).ok_or(LaunchError::GameBuild)?,
        (None, None) => return Err(LaunchError::NoScene),
    };
    let asset_root = args
        .asset_root
        .clone()
        .or_else(|| manifest.as_ref().map(|(m, dir)| m.asset_root(dir)));
    let title = args
        .title
        .clone()
        .or_else(|| manifest.as_ref().map(|(m, _)| m.name.clone()))
        .unwrap_or_else(|| title_of(&scene));
    let size = args.size.or_else(|| {
        manifest
            .as_ref()
            .map(|(m, _)| (m.window.width, m.window.height))
    });
    Ok(Launch {
        scene,
        asset_root,
        title,
        size,
    })
}

/// A scene's file name, for a window nothing else named.
///
/// Godot titles a running project after the project; with no manifest, the
/// scene is the closest thing to a name the player has.
fn title_of(scene: &Path) -> String {
    scene
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "Voltra".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"(
        version: 1,
        name: "Sandbox",
        scene: "scenes/sandbox.ron",
        window: (width: 960, height: 540),
    )"#;

    fn scratch_with_manifest() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("after 1970")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "voltra-launch-{nanos}-{:?}",
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        std::fs::write(dir.join(MANIFEST_FILE), MANIFEST).expect("writes");
        dir
    }

    #[test]
    fn a_scene_on_the_command_line_alone_runs_as_before() {
        let args = Args {
            scene: Some(PathBuf::from("levels/one.ron")),
            ..Args::default()
        };
        let dir = scratch_with_manifest();
        let launch = resolve(&args, Some(&dir)).expect("resolves");
        assert_eq!(launch.scene, PathBuf::from("levels/one.ron"));
        assert_eq!(launch.title, "one", "the manifest beside it is not read");
        assert_eq!(launch.asset_root, None);
    }

    #[test]
    fn no_arguments_runs_the_build_beside_the_player() {
        let dir = scratch_with_manifest();
        let launch = resolve(&Args::default(), Some(&dir)).expect("resolves");
        assert_eq!(launch.scene, dir.join("assets").join("scenes/sandbox.ron"));
        assert_eq!(launch.asset_root, Some(dir.join("assets")));
        assert_eq!(launch.title, "Sandbox");
        assert_eq!(launch.size, Some((960, 540)));
    }

    #[test]
    fn the_command_line_wins_over_the_manifest() {
        let dir = scratch_with_manifest();
        let args = Args {
            manifest: Some(dir.join(MANIFEST_FILE)),
            title: Some("Debug".to_owned()),
            size: Some((320, 240)),
            ..Args::default()
        };
        let launch = resolve(&args, None).expect("resolves");
        assert_eq!(launch.title, "Debug");
        assert_eq!(launch.size, Some((320, 240)));
        assert_eq!(launch.scene, dir.join("assets").join("scenes/sandbox.ron"));
    }

    #[test]
    fn nothing_at_all_is_an_error_that_says_what_is_missing() {
        let empty = std::env::temp_dir();
        let error = resolve(&Args::default(), Some(&empty.join("no-such-dir")));
        assert!(matches!(error, Err(LaunchError::NoScene)));
    }

    #[test]
    fn a_broken_manifest_is_reported_not_ignored() {
        let dir = scratch_with_manifest();
        std::fs::write(dir.join(MANIFEST_FILE), "(version: 1").expect("writes");
        let error = resolve(&Args::default(), Some(&dir));
        assert!(matches!(error, Err(LaunchError::Manifest(_))));
    }

    #[test]
    fn a_game_build_is_not_the_player_s_to_run() {
        let dir = scratch_with_manifest();
        std::fs::write(
            dir.join(MANIFEST_FILE),
            r#"(version: 1, name: "G", game: (package: "g"))"#,
        )
        .expect("writes");
        assert!(matches!(
            resolve(&Args::default(), Some(&dir)),
            Err(LaunchError::GameBuild)
        ));
    }
}
