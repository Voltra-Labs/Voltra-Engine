//! The player's files beside the slots: named text, or a value as RON,
//! through the same storage and the same rules a slot gets — a name that
//! cannot leave the folder, a write made whole or not at all, a bounded read.
//!
//! What the controls a player rebound
//! ([`ActionMap::save_player`](crate::ActionMap::save_player)) and an options
//! screen's choices are kept in: Godot's `user://settings.cfg` and Unity's
//! `PlayerPrefs` are the same idea, a small file per user that is not a save.

use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::Serialize;

use super::{SaveError, Saves, MAX_FILE_NAME_LEN, MAX_SAVE_BYTES};
use crate::storage;

impl Saves {
    /// Where the file `name` lives beside the slots: a file natively, a
    /// `localStorage` key on the web.
    pub fn path_of_file(&self, name: &str) -> Result<PathBuf, SaveError> {
        check_file_name(name)?;
        let dir = self.dir.as_ref().ok_or(SaveError::NoSaveDir)?;
        Ok(dir.join(name))
    }

    /// Writes `text` as the file `name`, replacing what was there — whole or
    /// not at all, as [`save`](Self::save) writes a slot.
    pub fn write_text(&self, name: &str, text: &str) -> Result<(), SaveError> {
        let path = self.path_of_file(name)?;
        storage::write(&path, text).map_err(|source| SaveError::Io {
            path: path.clone(),
            source,
        })?;
        log::info!("saved {}", path.display());
        Ok(())
    }

    /// What the file `name` holds, or `None` when there is no such file.
    /// One over [`MAX_SAVE_BYTES`] is refused before it is read.
    pub fn read_text(&self, name: &str) -> Result<Option<String>, SaveError> {
        let path = self.path_of_file(name)?;
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
        storage::read(&path).map(Some).map_err(io)
    }

    /// Removes the file `name`. Returns whether there was one.
    pub fn remove_file(&self, name: &str) -> Result<bool, SaveError> {
        let path = self.path_of_file(name)?;
        storage::remove(&path).map_err(|source| SaveError::Io { path, source })
    }

    /// Writes `data` as RON to the file `name`.
    pub fn save_file<T: Serialize>(&self, name: &str, data: &T) -> Result<(), SaveError> {
        let text = ron::ser::to_string_pretty(data, ron::ser::PrettyConfig::default())
            .map_err(SaveError::Serialize)?;
        self.write_text(name, &text)
    }

    /// What the file `name` holds as a `T`, or `None` with no such file. A
    /// file that is not a `T` is an error, as a slot's is.
    pub fn load_file<T: DeserializeOwned>(&self, name: &str) -> Result<Option<T>, SaveError> {
        let Some(text) = self.read_text(name)? else {
            return Ok(None);
        };
        ron::from_str(&text)
            .map(Some)
            .map_err(|source| SaveError::Parse {
                path: self
                    .path_of_file(name)
                    .expect("the name was checked by the read above"),
                source: Box::new(source),
            })
    }
}

/// A name that stays in the folder: no separator, no leading dot (so no
/// `..`), nothing a filesystem would refuse.
fn check_file_name(name: &str) -> Result<(), SaveError> {
    let valid = !name.is_empty()
        && name.len() <= MAX_FILE_NAME_LEN
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if valid {
        Ok(())
    } else {
        Err(SaveError::BadFileName(name.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use voltra_testkit::scratch_root;

    use super::*;

    #[test]
    fn a_file_beside_the_slots_reads_back_and_is_not_a_slot() {
        let saves = Saves::in_dir(scratch_root());
        saves
            .write_text("controls.input.ron", "(version: 1)")
            .expect("writes");
        assert_eq!(
            saves.read_text("controls.input.ron").expect("reads"),
            Some("(version: 1)".to_owned())
        );
        assert!(saves.slots().is_empty(), "a player's file is not a slot");
        assert!(saves.remove_file("controls.input.ron").expect("removes"));
        assert_eq!(saves.read_text("controls.input.ron").expect("reads"), None);
    }

    #[test]
    fn a_value_round_trips_through_a_file() {
        let saves = Saves::in_dir(scratch_root());
        saves
            .save_file("options.settings.ron", &(1u32, true))
            .expect("saves");
        assert_eq!(
            saves
                .load_file::<(u32, bool)>("options.settings.ron")
                .expect("loads"),
            Some((1, true))
        );
        assert!(matches!(
            saves.load_file::<String>("options.settings.ron"),
            Err(SaveError::Parse { .. })
        ));
    }

    #[test]
    fn a_file_name_cannot_leave_the_folder() {
        let saves = Saves::in_dir(scratch_root());
        for bad in ["../up", "a/b", "a\\b", "..", ".hidden", "", "C:x"] {
            assert!(
                matches!(saves.write_text(bad, "x"), Err(SaveError::BadFileName(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn with_no_folder_a_file_says_so() {
        let saves = Saves::default();
        assert!(matches!(
            saves.read_text("controls.input.ron"),
            Err(SaveError::NoSaveDir)
        ));
    }
}
