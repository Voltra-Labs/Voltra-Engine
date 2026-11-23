//! Where a game's per-user files go, by platform.
//!
//! The folders every platform's guidelines name for application data, the
//! ones Godot's `user://`, Unity's `persistentDataPath` and the `dirs` crate
//! resolve to — worked out from the environment here rather than through a
//! dependency, since it is three rules.

use std::ffi::OsString;
use std::path::PathBuf;

/// Overrides every rule below: a portable build keeping saves beside itself,
/// or a test keeping them out of the real profile.
pub const SAVE_DIR_ENV: &str = "VOLTRA_SAVE_DIR";

/// The platforms whose rules differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
    /// Linux and the other Unixes: the XDG base directories.
    Unix,
    /// A browser: no folders, only the page's `localStorage`, under keys
    /// that read as the path below (`/<game>/saves/<slot>.save.ron`).
    Web,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_arch = "wasm32") {
            Self::Web
        } else if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Unix
        }
    }
}

/// `game`'s saves folder on this machine, or `None` when the environment
/// names no home to put it in.
pub fn saves_dir(game: &str) -> Option<PathBuf> {
    resolve(Platform::current(), game, |key| std::env::var_os(key))
}

/// The rules, with the platform and environment passed in so they can be
/// tested without touching process-global state.
///
/// An environment path that is not absolute is ignored, as the XDG spec
/// requires: a relative one would put saves wherever the game was launched
/// from.
pub fn resolve(
    platform: Platform,
    game: &str,
    env: impl Fn(&str) -> Option<OsString>,
) -> Option<PathBuf> {
    let absolute = |key: &str| {
        env(key)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    if let Some(dir) = absolute(SAVE_DIR_ENV) {
        return Some(dir);
    }
    let folder = safe_file_name(game);
    let folder = if folder.is_empty() {
        "Voltra".to_owned()
    } else {
        folder
    };
    let base = match platform {
        // The storage is the page's own, so there is no home to find.
        Platform::Web => PathBuf::from("/"),
        Platform::Windows => absolute("APPDATA")?,
        Platform::MacOs => absolute("HOME")?.join("Library/Application Support"),
        Platform::Unix => absolute("XDG_DATA_HOME")
            .or_else(|| absolute("HOME").map(|home| home.join(".local/share")))?,
    };
    Some(base.join(folder).join("saves"))
}

/// `name` reduced to what every filesystem accepts in a file or folder name:
/// letters, digits, `-`, `_` and single spaces, trimmed. Empty when nothing
/// in it survives.
pub fn safe_file_name(name: &str) -> String {
    let kept: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                ' '
            }
        })
        .collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, OsString> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), OsString::from(v)))
            .collect();
        move |key| map.get(key).cloned()
    }

    fn root(path: &str) -> String {
        if cfg!(windows) {
            format!("C:{path}")
        } else {
            path.to_owned()
        }
    }

    #[test]
    fn web_saves_are_keyed_by_game_with_no_environment() {
        assert_eq!(
            resolve(Platform::Web, "Sky Rift", env(&[])),
            Some(PathBuf::from("/").join("Sky Rift").join("saves"))
        );
    }

    #[test]
    fn windows_saves_go_under_appdata() {
        let appdata = root("/Users/p/AppData/Roaming");
        let dir = resolve(Platform::Windows, "Sky Rift", env(&[("APPDATA", &appdata)]));
        assert_eq!(
            dir,
            Some(PathBuf::from(&appdata).join("Sky Rift").join("saves"))
        );
    }

    #[test]
    fn unix_prefers_xdg_and_falls_back_to_home() {
        let xdg = root("/home/p/.data");
        let home = root("/home/p");
        let with_xdg = resolve(
            Platform::Unix,
            "Game",
            env(&[("XDG_DATA_HOME", &xdg), ("HOME", &home)]),
        );
        assert_eq!(
            with_xdg,
            Some(PathBuf::from(&xdg).join("Game").join("saves"))
        );
        let without = resolve(Platform::Unix, "Game", env(&[("HOME", &home)]));
        assert_eq!(
            without,
            Some(
                PathBuf::from(&home)
                    .join(".local/share")
                    .join("Game")
                    .join("saves")
            )
        );
    }

    #[test]
    fn macos_uses_application_support() {
        let home = root("/Users/p");
        let dir = resolve(Platform::MacOs, "Game", env(&[("HOME", &home)]));
        assert_eq!(
            dir,
            Some(
                PathBuf::from(&home)
                    .join("Library/Application Support")
                    .join("Game")
                    .join("saves")
            )
        );
    }

    #[test]
    fn the_override_wins_and_a_relative_one_is_ignored() {
        let custom = root("/portable/saves");
        let dir = resolve(
            Platform::Windows,
            "Game",
            env(&[(SAVE_DIR_ENV, &custom), ("APPDATA", &root("/a"))]),
        );
        assert_eq!(dir, Some(PathBuf::from(&custom)));
        let relative = resolve(
            Platform::Windows,
            "Game",
            env(&[(SAVE_DIR_ENV, "saves"), ("APPDATA", &root("/a"))]),
        );
        assert_eq!(
            relative,
            Some(PathBuf::from(root("/a")).join("Game").join("saves"))
        );
    }

    #[test]
    fn no_home_at_all_is_no_saves_dir() {
        assert_eq!(resolve(Platform::Unix, "Game", env(&[])), None);
    }

    #[test]
    fn a_game_name_is_made_safe_as_a_folder() {
        let appdata = root("/a");
        let dir = resolve(
            Platform::Windows,
            "../../evil: game",
            env(&[("APPDATA", &appdata)]),
        );
        assert_eq!(
            dir,
            Some(PathBuf::from(&appdata).join("evil game").join("saves"))
        );
    }

    #[test]
    fn safe_file_name_keeps_letters_and_drops_the_rest() {
        assert_eq!(safe_file_name("  My: Game/2 *final*  "), "My Game 2 final");
        assert_eq!(safe_file_name("???"), "");
    }
}
