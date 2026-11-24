//! Save games: named slots holding whatever the game serialises — and,
//! beside them, the player's other files: the controls they rebound, the
//! options they chose.
//!
//! Unreal's shape — `SaveGameToSlot` with a game-defined `USaveGame` — rather
//! than a snapshot of the world. A world holds what the *level* is, and much
//! of it is rebuilt from the scene file anyway; what a save needs is what the
//! *player* did, which only the game knows (the coins counted, the level
//! reached, the door opened). So the game hands over a `Serialize` type and
//! the engine owns the rest: where the file lives, its name, writing it so a
//! crash cannot leave half of it, and reading it back without trusting it.

pub mod dir;
mod error;
mod files;

pub use error::SaveError;

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

/// The extension every slot file carries.
pub const EXTENSION: &str = "save.ron";

/// The most a slot — or any other of the player's files — may hold. A save
/// is a file the player can edit, copy in from a forum or have corrupted by
/// a full disk; reading one is never an unbounded allocation.
pub const MAX_SAVE_BYTES: u64 = 16 * 1024 * 1024;

/// The longest slot name accepted.
pub const MAX_SLOT_LEN: usize = 64;

/// The longest name accepted for a player's file beside the slots
/// ([`Saves::write_text`]).
pub const MAX_FILE_NAME_LEN: usize = 96;

/// A game's save slots, in one folder — and the player's other files,
/// beside them ([`write_text`](Self::write_text), [`save_file`](Self::save_file)).
///
/// One folder for all of a player's files, slots or not: the environment
/// override that moves saves ([`dir::SAVE_DIR_ENV`]) moves the rest with
/// them, so a portable build or a test keeps every one of them out of the
/// real profile.
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

    /// Where `slot` lives: a file natively, a `localStorage` key on the web.
    pub fn path_of(&self, slot: &str) -> Result<PathBuf, SaveError> {
        check_slot(slot)?;
        self.path_of_file(&slot_file(slot))
    }

    /// Writes `data` to `slot`, replacing what was there. Natively through a
    /// temporary file and a rename, so a crash mid-write leaves the previous
    /// save rather than half of this one; on the web one `localStorage`
    /// write, which the browser already makes whole or not at all.
    pub fn save<T: Serialize>(&self, slot: &str, data: &T) -> Result<(), SaveError> {
        check_slot(slot)?;
        self.save_file(&slot_file(slot), data)
    }

    /// What `slot` holds, or `None` if nothing was ever saved there.
    ///
    /// A file that will not parse as `T` is an error, not `None`: a save
    /// from an older version of the game is something to tell the player
    /// about, not to silently start over from.
    pub fn load<T: DeserializeOwned>(&self, slot: &str) -> Result<Option<T>, SaveError> {
        check_slot(slot)?;
        self.load_file(&slot_file(slot))
    }

    /// Removes `slot`. Returns whether there was one.
    pub fn delete(&self, slot: &str) -> Result<bool, SaveError> {
        check_slot(slot)?;
        self.remove_file(&slot_file(slot))
    }

    /// Every slot saved so far, sorted. Files that are not slots — a
    /// temporary left by a crash, anything a player dropped in the folder —
    /// are not listed.
    pub fn slots(&self) -> Vec<String> {
        let Some(dir) = self.dir.as_ref() else {
            return Vec::new();
        };
        let suffix = format!(".{EXTENSION}");
        let mut slots: Vec<String> = crate::storage::file_names(dir)
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

/// The file `slot` is kept in.
fn slot_file(slot: &str) -> String {
    format!("{slot}.{EXTENSION}")
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
