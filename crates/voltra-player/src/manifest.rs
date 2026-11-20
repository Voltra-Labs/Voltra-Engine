//! What a build is: the scene it starts in, its window, and where its assets
//! are.
//!
//! The project settings file stages 19 and 21 both turned down, earning its
//! place now because it describes *a build*, not an editor — Godot's export
//! preset, Unity's build settings. The player reads it when no command line
//! says otherwise; `cargo xtask dist` reads it to know what to package and
//! writes the copy that ships.

use std::fmt;
use std::path::{Component, Path, PathBuf};

use voltra_assets::AssetPath;

/// The manifest's file name, beside the player executable in a build and at
/// the workspace root in the source tree.
pub const MANIFEST_FILE: &str = "build.ron";

/// The format version this build reads and writes.
pub const VERSION: u32 = 1;

/// A build, as `build.ron` describes it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub version: u32,
    /// The game's name: the window title, the build folder, the executable.
    pub name: String,
    /// The scene to start in, relative to the asset root. Needed when the
    /// build runs the player; a game with its own binary builds its own
    /// start and may leave it out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<String>,
    #[serde(default)]
    pub window: Window,
    /// The asset root, relative to the manifest's own directory.
    #[serde(default = "default_assets")]
    pub assets: PathBuf,
    /// The game's own binary, for a game with code: what `cargo xtask dist`
    /// builds and ships in place of the player.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game: Option<GameBinary>,
}

/// Which cargo target is the game: `bin` or `example` of `package`, or the
/// package's own binary when both are left out.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GameBinary {
    pub package: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
}

impl GameBinary {
    /// The name cargo gives the built executable, without its suffix.
    pub fn target_name(&self) -> &str {
        self.example
            .as_deref()
            .or(self.bin.as_deref())
            .unwrap_or(&self.package)
    }

    /// Whether it builds under `target/<profile>/examples` rather than
    /// beside the other binaries.
    pub fn is_example(&self) -> bool {
        self.example.is_some()
    }

    /// The `cargo build` arguments that select it.
    pub fn cargo_args(&self) -> Vec<String> {
        let mut args = vec!["--package".to_owned(), self.package.clone()];
        if let Some(example) = &self.example {
            args.extend(["--example".to_owned(), example.clone()]);
        } else if let Some(bin) = &self.bin {
            args.extend(["--bin".to_owned(), bin.clone()]);
        }
        args
    }
}

/// What every name handed to cargo may hold: a typo'd shell character in a
/// manifest must not become part of a command line.
fn is_cargo_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// RON with `Some` implied, so `scene: "a.ron"` reads as it did when the
/// field was not optional, and a manifest written before stays valid.
fn ron_options() -> ron::Options {
    ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
}

/// The window a build opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Window {
    pub width: u32,
    pub height: u32,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
        }
    }
}

fn default_assets() -> PathBuf {
    PathBuf::from("assets")
}

/// Why a manifest could not be used.
#[derive(Debug)]
pub enum ManifestError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: Box<ron::error::SpannedError>,
    },
    Serialize(ron::Error),
    /// Valid RON describing something that cannot be a build.
    Invalid {
        path: PathBuf,
        reason: String,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "could not access {}: {source}", path.display()),
            Self::Parse { path, source } => {
                write!(f, "could not parse {}: {source}", path.display())
            }
            Self::Serialize(source) => write!(f, "could not write a manifest: {source}"),
            Self::Invalid { path, reason } => {
                write!(f, "{} is not a usable build: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for ManifestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Serialize(source) => Some(source),
            Self::Invalid { .. } => None,
        }
    }
}

impl Manifest {
    /// Parses and checks a manifest. `path` only names the source in errors.
    pub fn from_ron(text: &str, path: &Path) -> Result<Self, ManifestError> {
        let manifest: Self =
            ron_options()
                .from_str(text)
                .map_err(|source| ManifestError::Parse {
                    path: path.to_path_buf(),
                    source: Box::new(source),
                })?;
        manifest.check().map_err(|reason| ManifestError::Invalid {
            path: path.to_path_buf(),
            reason,
        })?;
        Ok(manifest)
    }

    pub fn load(path: &Path) -> Result<Self, ManifestError> {
        let text = std::fs::read_to_string(path).map_err(|source| ManifestError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_ron(&text, path)
    }

    pub fn to_ron(&self) -> Result<String, ManifestError> {
        ron_options()
            .to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(ManifestError::Serialize)
    }

    /// Everything a build needs to be true before anything is built from it.
    fn check(&self) -> Result<(), String> {
        if self.version != VERSION {
            return Err(format!(
                "version {} (this build reads {VERSION})",
                self.version
            ));
        }
        if self.file_name().is_empty() {
            return Err(format!(
                "the name `{}` has nothing usable as a file name",
                self.name
            ));
        }
        match (&self.scene, &self.game) {
            (Some(scene), _) => {
                AssetPath::new(scene).map_err(|e| format!("scene: {e}"))?;
            }
            (None, None) => {
                return Err("it names neither a start scene nor a game binary".to_owned());
            }
            (None, Some(_)) => {}
        }
        if let Some(game) = &self.game {
            if game.bin.is_some() && game.example.is_some() {
                return Err("the game names both a bin and an example; pick one".to_owned());
            }
            let names = std::iter::once(&game.package)
                .chain(game.bin.as_ref())
                .chain(game.example.as_ref());
            for name in names {
                if !is_cargo_name(name) {
                    return Err(format!(
                        "`{name}` is not a cargo target name: letters, digits, `-` and `_`"
                    ));
                }
            }
        }
        if self.window.width == 0 || self.window.height == 0 {
            return Err("the window has no size".to_owned());
        }
        // Relative, so the build folder can be moved and still find its
        // assets; `..` is allowed in the source tree's manifest, which sits
        // beside the asset folder rather than above it in some layouts.
        if self.assets.is_absolute()
            || self
                .assets
                .components()
                .any(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
        {
            return Err(format!(
                "assets must be relative to the manifest, got {}",
                self.assets.display()
            ));
        }
        Ok(())
    }

    /// The asset root, for a manifest read from `manifest_dir`.
    pub fn asset_root(&self, manifest_dir: &Path) -> PathBuf {
        manifest_dir.join(&self.assets)
    }

    /// The start scene's path on disk, for a manifest read from
    /// `manifest_dir`.
    pub fn scene_path(&self, manifest_dir: &Path) -> Option<PathBuf> {
        self.scene
            .as_ref()
            .map(|scene| self.asset_root(manifest_dir).join(scene))
    }

    /// The name, reduced to what every filesystem accepts in a file or
    /// folder name — the rule the saves folder is named by too, so a build
    /// and its saves agree on what the game is called.
    pub fn file_name(&self) -> String {
        voltra_core::saves::dir::safe_file_name(&self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r#"(
        version: 1,
        name: "Sandbox",
        scene: "scenes/sandbox.ron",
        window: (width: 960, height: 540),
    )"#;

    fn parsed(text: &str) -> Result<Manifest, ManifestError> {
        Manifest::from_ron(text, Path::new("build.ron"))
    }

    #[test]
    fn a_manifest_reads_with_its_assets_defaulted() {
        let manifest = parsed(FILE).expect("parses");
        assert_eq!(manifest.name, "Sandbox");
        assert_eq!(manifest.assets, PathBuf::from("assets"));
        assert_eq!(
            manifest.scene_path(Path::new("game")),
            Some(Path::new("game").join("assets").join("scenes/sandbox.ron"))
        );
    }

    #[test]
    fn a_manifest_round_trips() {
        let manifest = parsed(FILE).expect("parses");
        let text = manifest.to_ron().expect("writes");
        assert_eq!(parsed(&text).expect("reads back"), manifest);
    }

    #[test]
    fn a_scene_outside_the_asset_root_is_refused() {
        let text = FILE.replace("scenes/sandbox.ron", "../secret.ron");
        assert!(matches!(parsed(&text), Err(ManifestError::Invalid { .. })));
    }

    #[test]
    fn an_absolute_asset_root_is_refused() {
        let absolute = if cfg!(windows) {
            r"C:\\assets"
        } else {
            "/assets"
        };
        let text = FILE.replace("window:", &format!("assets: \"{absolute}\", window:"));
        assert!(matches!(parsed(&text), Err(ManifestError::Invalid { .. })));
    }

    #[test]
    fn a_zero_sized_window_is_refused() {
        let text = FILE.replace("width: 960", "width: 0");
        assert!(matches!(parsed(&text), Err(ManifestError::Invalid { .. })));
    }

    #[test]
    fn another_version_is_refused() {
        let text = FILE.replace("version: 1", "version: 2");
        assert!(matches!(parsed(&text), Err(ManifestError::Invalid { .. })));
    }

    #[test]
    fn a_name_becomes_a_safe_file_name() {
        let mut manifest = parsed(FILE).expect("parses");
        manifest.name = "  My: Game/2 *final*  ".to_owned();
        assert_eq!(manifest.file_name(), "My Game 2 final");
        manifest.name = "???".to_owned();
        assert_eq!(manifest.file_name(), "");
    }

    #[test]
    fn a_game_binary_reads_and_round_trips() {
        let text = r#"(
            version: 1,
            name: "Platformer",
            game: (package: "voltra-core", example: "platformer"),
        )"#;
        let manifest = parsed(text).expect("parses");
        let game = manifest.game.clone().expect("a game");
        assert_eq!(game.target_name(), "platformer");
        assert!(game.is_example());
        assert_eq!(
            game.cargo_args(),
            vec!["--package", "voltra-core", "--example", "platformer"]
        );
        assert_eq!(manifest.scene, None);
        let again = parsed(&manifest.to_ron().expect("writes")).expect("reads back");
        assert_eq!(again, manifest);
    }

    #[test]
    fn neither_scene_nor_game_is_refused() {
        let text = r#"(version: 1, name: "Nothing")"#;
        assert!(matches!(parsed(text), Err(ManifestError::Invalid { .. })));
    }

    #[test]
    fn a_game_naming_both_bin_and_example_is_refused() {
        let text = r#"(version: 1, name: "G", game: (package: "p", bin: "a", example: "b"))"#;
        assert!(matches!(parsed(text), Err(ManifestError::Invalid { .. })));
    }

    #[test]
    fn a_target_name_that_is_not_a_cargo_name_is_refused() {
        let text = r#"(version: 1, name: "G", game: (package: "p; rm -rf /"))"#;
        assert!(matches!(parsed(text), Err(ManifestError::Invalid { .. })));
    }

    #[test]
    fn a_package_binary_is_named_after_the_package() {
        let game = GameBinary {
            package: "my-game".to_owned(),
            bin: None,
            example: None,
        };
        assert_eq!(game.target_name(), "my-game");
        assert_eq!(game.cargo_args(), vec!["--package", "my-game"]);
    }

    #[test]
    fn a_name_with_nothing_usable_is_refused() {
        let text = FILE.replace("\"Sandbox\"", "\"///\"");
        assert!(matches!(parsed(&text), Err(ManifestError::Invalid { .. })));
    }
}
