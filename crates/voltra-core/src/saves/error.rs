//! Why a save — or any of a player's files — could not be written, read or
//! removed.

use std::fmt;
use std::path::PathBuf;

use super::{dir, MAX_FILE_NAME_LEN, MAX_SAVE_BYTES, MAX_SLOT_LEN};

/// Why a save, or another of a player's files, could not be written, read
/// or removed.
#[derive(Debug)]
pub enum SaveError {
    /// The environment names no home for a saves folder; see
    /// [`dir::SAVE_DIR_ENV`].
    NoSaveDir,
    /// A slot name that is empty, too long, or not just letters, digits,
    /// `-` and `_` — which is what keeps a slot inside the saves folder.
    BadSlot(String),
    /// A file name that is empty, too long, starts with a dot, or is not
    /// just letters, digits, `.`, `-` and `_` — which keeps a player's file
    /// inside the folder, as a slot name does.
    BadFileName(String),
    /// The slot file is over [`MAX_SAVE_BYTES`].
    TooLarge {
        /// The slot file.
        path: PathBuf,
        /// Its size in bytes.
        bytes: u64,
    },
    /// The slot file could not be read, written or removed.
    Io {
        /// The file involved.
        path: PathBuf,
        /// The underlying I/O error.
        source: std::io::Error,
    },
    /// The value could not be written as RON.
    Serialize(ron::Error),
    /// The slot file is not RON for the type asked for.
    Parse {
        /// The file that did not parse.
        path: PathBuf,
        /// The parser's error, with its position.
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
            Self::BadFileName(name) => write!(
                f,
                "`{name}` is not a file name: 1 to {MAX_FILE_NAME_LEN} letters, digits, `.`, `-` or `_`, not starting with `.`"
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
