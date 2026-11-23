//! Save games: named slots holding whatever the game serialises.
//!
//! Unreal's shape — `SaveGameToSlot` with a game-defined `USaveGame` — rather
//! than a snapshot of the world. A world holds what the *level* is, and much
//! of it is rebuilt from the scene file anyway; what a save needs is what the
//! *player* did, which only the game knows (the coins counted, the level
//! reached, the door opened). So the game hands over a `Serialize` type and
//! the engine owns the rest: where the file lives, its name, writing it so a
//! crash cannot leave half of it, and reading it back without trusting it.

pub mod dir;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
use native as storage;
#[cfg(target_arch = "wasm32")]
use web as storage;

use std::fmt;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

/// The extension every slot file carries.
pub const EXTENSION: &str = "save.ron";

/// The most a slot may hold. A save is a file the player can edit, copy in
/// from a forum or have corrupted by a full disk; reading one is never an
/// unbounded allocation.
pub const MAX_SAVE_BYTES: u64 = 16 * 1024 * 1024;

/// The longest slot name accepted.
pub const MAX_SLOT_LEN: usize = 64;

/// Why a save could not be written, read or removed.
#[derive(Debug)]
pub enum SaveError {
    /// The environment names no home for a saves folder; see
    /// [`dir::SAVE_DIR_ENV`].
    NoSaveDir,
    /// A slot name that is empty, too long, or not just letters, digits,
    /// `-` and `_` — which is what keeps a slot inside the saves folder.
    BadSlot(String),
    TooLarge {
        path: PathBuf,
        bytes: u64,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Serialize(ron::Error),
    Parse {
        path: PathBuf,
        source: Box<ron::error::SpannedError>,
    },
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSaveDir => write!(
                f,
                "no folder to save into: the environment names no home (set {})",
                dir::SAVE_DIR_ENV
            ),
            Self::BadSlot(slot) => write!(
                f,
                "`{slot}` is not a slot name: 1 to {MAX_SLOT_LEN} letters, digits, `-` or `_`"
            ),
            Self::TooLarge { path, bytes } => write!(
                f,
                "{} is {bytes} bytes, over the {MAX_SAVE_BYTES}-byte limit",
                path.display()
            ),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Serialize(source) => write!(f, "could not write the save: {source}"),
            Self::Parse { path, source } => {
                write!(
                    f,
                    "{} is not a save this game can read: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for SaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Serialize(source) => Some(source),
            Self::Parse { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// A game's save slots, in one folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Saves {
    dir: Option<PathBuf>,
}

impl Saves {
    /// The per-user saves folder for `game` — see [`dir::saves_dir`].
    pub fn for_game(game: &str) -> Self {
        Self {
            dir: dir::saves_dir(game),
        }
    }

    /// Slots in `dir`, for a portable build or a test.
    pub fn in_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: Some(dir.into()),
        }
    }

    /// The folder, if the environment gave one.
    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// Where `slot` lives — also what a game hands
    /// [`ActionMap::save`](crate::ActionMap::save) to keep a player's
    /// bindings beside their saves.
    pub fn path_of(&self, slot: &str) -> Result<PathBuf, SaveError> {
        check_slot(slot)?;
        let dir = self.dir.as_ref().ok_or(SaveError::NoSaveDir)?;
        Ok(dir.join(format!("{slot}.{EXTENSION}")))
    }

    /// Writes `data` to `slot`, replacing what was there. Natively through a
    /// temporary file and a rename, so a crash mid-write leaves the previous
    /// save rather than half of this one; on the web one `localStorage`
    /// write, which the browser already makes whole or not at all.
    pub fn save<T: Serialize>(&self, slot: &str, data: &T) -> Result<(), SaveError> {
        let path = self.path_of(slot)?;
        let text = ron::ser::to_string_pretty(data, ron::ser::PrettyConfig::default())
            .map_err(SaveError::Serialize)?;
        storage::write(&path, &text).map_err(|source| SaveError::Io {
            path: path.clone(),
            source,
        })?;
        log::info!("saved {}", path.display());
        Ok(())
    }

    /// What `slot` holds, or `None` if nothing was ever saved there.
    ///
    /// A file that will not parse as `T` is an error, not `None`: a save
    /// from an older version of the game is something to tell the player
    /// about, not to silently start over from.
    pub fn load<T: DeserializeOwned>(&self, slot: &str) -> Result<Option<T>, SaveError> {
        let path = self.path_of(slot)?;
        let io = |source| SaveError::Io {
            path: path.clone(),
            source,
        };
        let Some(bytes) = storage::size(&path).map_err(io)? else {
            return Ok(None);
        };
        if bytes > MAX_SAVE_BYTES {
            return Err(SaveError::TooLarge { path, bytes });
        }
        let text = storage::read(&path).map_err(io)?;
        ron::from_str(&text)
            .map(Some)
            .map_err(|source| SaveError::Parse {
                path,
                source: Box::new(source),
            })
    }

    /// Removes `slot`. Returns whether there was one.
    pub fn delete(&self, slot: &str) -> Result<bool, SaveError> {
        let path = self.path_of(slot)?;
        storage::remove(&path).map_err(|source| SaveError::Io { path, source })
    }

    /// Every slot saved so far, sorted. Files that are not slots — a
    /// temporary left by a crash, anything a player dropped in the folder —
    /// are not listed.
    pub fn slots(&self) -> Vec<String> {
        let Some(dir) = self.dir.as_ref() else {
            return Vec::new();
        };
        let suffix = format!(".{EXTENSION}");
        let mut slots: Vec<String> = storage::file_names(dir)
            .into_iter()
            .filter_map(|name| {
                let slot = name.strip_suffix(&suffix)?;
                check_slot(slot).ok()?;
                Some(slot.to_owned())
            })
            .collect();
        slots.sort();
        slots
    }
}

fn check_slot(slot: &str) -> Result<(), SaveError> {
    let valid = !slot.is_empty()
        && slot.len() <= MAX_SLOT_LEN
        && slot
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(SaveError::BadSlot(slot.to_owned()))
    }
}

#[cfg(test)]
mod tests;
