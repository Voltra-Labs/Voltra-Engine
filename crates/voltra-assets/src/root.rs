//! Where an [`AssetPath`](crate::AssetPath) is resolved from.
//!
//! Nothing here reads a file's contents; this decides which directory the
//! paths in a scene are relative to, once, at startup.
//!
//! No engine resolves this against the process working directory, and neither
//! does this one: a working directory is whatever shell or launcher started
//! the process, and it changes the meaning of every path in every scene file.
//! Bevy resolves `BEVY_ASSET_ROOT`, then `CARGO_MANIFEST_DIR`, then the
//! executable's parent; Unreal hangs everything off the executable's base
//! directory; Unity's `Application.dataPath` is `<project>/Assets` in the
//! editor and `<exe>_Data` in a build; Godot's `res://` is the project
//! directory in the editor and the PCK beside the executable once exported.
//! The order below is that shape, with the working directory kept only as the
//! answer of last resort — something has to be returned, and a wrong root
//! surfaces as "texture failed to load", which is a logged, recoverable state
//! rather than a panic.

use std::path::{Path, PathBuf};

/// Environment variable that overrides every other rule.
///
/// Named for this engine rather than reusing anyone else's: a shell that
/// already exports `BEVY_ASSET_ROOT` for another project must not reach into
/// ours.
pub const ROOT_ENV: &str = "VOLTRA_ASSET_ROOT";

/// The directory name looked for while walking upwards.
const ASSETS_DIR: &str = "assets";

/// How many levels above the starting directory the walk may look.
///
/// Bounded rather than "up to the filesystem root": an unbounded walk from a
/// temporary directory would adopt any `assets` directory sitting near the
/// drive root and silently resolve every scene against a stranger's files.
/// Six covers `target/debug/deps` under a workspace member, which is the
/// deepest layout this repository produces.
const MAX_ASCENT: usize = 6;

/// The root every [`AssetPath`](crate::AssetPath) is joined onto, for a caller
/// that has not been told one.
///
/// [`App::with_asset_root`](../../voltra_core/struct.App.html) overrides this;
/// this is what it falls back to.
pub fn default_root() -> PathBuf {
    resolve_root(
        std::env::var_os(ROOT_ENV).map(PathBuf::from),
        std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from),
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf)),
        &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    )
}

/// The resolution rules, with every input passed in.
///
/// Separated from [`default_root`] so the rules can be tested without touching
/// process-global state: `std::env::set_var` is visible to every thread, and
/// cargo runs unit tests on many.
///
/// - `env_override` is taken verbatim, existing or not. Someone who set it
///   meant it, and silently ignoring a typo would be worse than failing to
///   load the textures under it.
/// - `manifest_dir` is set by `cargo run` and `cargo test`. It names the
///   *member* crate in a workspace, which is why the search walks upwards
///   rather than joining `assets` onto it.
/// - `exe_dir` is the shipped-binary case, and gets the same upward walk so a
///   `target/debug/voltra-editor.exe` still finds the repository's assets.
/// - `cwd` is the last resort.
pub fn resolve_root(
    env_override: Option<PathBuf>,
    manifest_dir: Option<PathBuf>,
    exe_dir: Option<PathBuf>,
    cwd: &Path,
) -> PathBuf {
    if let Some(root) = env_override {
        return root;
    }

    manifest_dir
        .as_deref()
        .and_then(ascend_to_assets)
        .or_else(|| exe_dir.as_deref().and_then(ascend_to_assets))
        .unwrap_or_else(|| cwd.join(ASSETS_DIR))
}

/// The nearest `assets` directory at or above `start`, within [`MAX_ASCENT`].
fn ascend_to_assets(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .take(MAX_ASCENT + 1)
        .map(|dir| dir.join(ASSETS_DIR))
        .find(|candidate| candidate.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch tree with an `assets` directory `depth` levels above `start`.
    ///
    /// Returns `(start_dir, expected_assets_dir)`.
    fn tree_with_assets_above(depth: usize) -> (PathBuf, PathBuf) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("the clock is after 1970")
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "voltra-root-{nanos}-{:?}",
            std::thread::current().id()
        ));
        let assets = base.join("assets");
        std::fs::create_dir_all(&assets).expect("assets dir");

        let mut start = base.clone();
        for level in 0..depth {
            start = start.join(format!("level{level}"));
        }
        std::fs::create_dir_all(&start).expect("start dir");

        (start, assets)
    }

    #[test]
    fn the_environment_override_wins_over_everything() {
        let (start, assets) = tree_with_assets_above(1);
        let forced = start.join("somewhere-else");

        let root = resolve_root(
            Some(forced.clone()),
            Some(start.clone()),
            Some(start),
            Path::new("."),
        );

        assert_eq!(root, forced);
        assert_ne!(root, assets, "the override must not be second-guessed");
    }

    #[test]
    fn the_manifest_directory_is_searched_before_the_executable() {
        // `cargo run -p voltra-editor` sets CARGO_MANIFEST_DIR to the *member*
        // crate, not the workspace root, so the walk upwards is the whole
        // point: `crates/voltra-editor` has no `assets` and the root does.
        let (manifest, assets) = tree_with_assets_above(2);
        let (exe, other_assets) = tree_with_assets_above(1);

        let root = resolve_root(None, Some(manifest), Some(exe), Path::new("."));

        assert_eq!(root, assets);
        assert_ne!(root, other_assets);
    }

    #[test]
    fn the_executable_directory_is_used_when_there_is_no_manifest() {
        let (exe, assets) = tree_with_assets_above(0);

        let root = resolve_root(None, None, Some(exe), Path::new("."));

        assert_eq!(root, assets);
    }

    #[test]
    fn a_directory_with_no_assets_anywhere_falls_back_to_the_cwd() {
        let (start, _) = tree_with_assets_above(0);
        let barren = start.join("no-assets-here");
        std::fs::create_dir_all(&barren).expect("barren dir");
        let cwd = Path::new("/some/working/dir");

        // `barren`'s parent does hold an `assets`, so walk from a tree that
        // has none at all: a fresh temp dir with nothing above it we control.
        let lonely = std::env::temp_dir().join("voltra-root-lonely-nonexistent");
        let root = resolve_root(None, None, Some(lonely), cwd);

        assert_eq!(root, cwd.join("assets"));
    }

    #[test]
    fn the_walk_upwards_is_bounded() {
        // MAX_ASCENT levels up is found; one more is not. An unbounded walk
        // from a temp directory would happily adopt an `assets` sitting near
        // the drive root and resolve every path against a stranger's files.
        let (deep, assets) = tree_with_assets_above(MAX_ASCENT);
        assert_eq!(
            resolve_root(None, None, Some(deep), Path::new("/cwd")),
            assets
        );

        let (too_deep, _) = tree_with_assets_above(MAX_ASCENT + 1);
        assert_eq!(
            resolve_root(None, None, Some(too_deep), Path::new("/cwd")),
            Path::new("/cwd").join("assets"),
        );
    }

    #[test]
    fn the_default_root_is_absolute_in_this_workspace() {
        // Unit tests run with CARGO_MANIFEST_DIR set to voltra-assets, whose
        // grandparent holds the repository's `assets`.
        let root = default_root();
        assert!(root.is_absolute(), "got {root:?}");
        assert!(root.ends_with("assets"), "got {root:?}");
    }
}
