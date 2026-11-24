//! One language's strings, as its file holds them.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

/// The format a table file is in.
pub const TABLE_VERSION: u32 = 1;

/// `locale/<language>.ron`:
///
/// ```ron
/// (
///     version: 1,
///     strings: {
///         "menu.play": "Jugar",
///         "dialogue.intro": "Hola, viajero.",
///     },
/// )
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StringTable {
    pub version: u32,
    #[serde(default)]
    pub strings: HashMap<String, String>,
}

/// Why a table did not read.
#[derive(Debug)]
pub enum TableError {
    Io(std::io::Error),
    Parse(ron::error::SpannedError),
    /// Written by a newer engine.
    Version(u32),
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "cannot read it: {e}"),
            Self::Parse(e) => write!(f, "cannot parse it: {e}"),
            Self::Version(v) => write!(f, "version {v} is newer than {TABLE_VERSION}"),
        }
    }
}

impl std::error::Error for TableError {}

impl StringTable {
    /// Reads the table at `path`, through the asset funnel.
    pub fn load(path: &Path) -> Result<Self, TableError> {
        let text = voltra_assets::fs::read_to_string(path).map_err(TableError::Io)?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self, TableError> {
        let table: Self = ron::from_str(text).map_err(TableError::Parse)?;
        if table.version > TABLE_VERSION {
            return Err(TableError::Version(table.version));
        }
        Ok(table)
    }
}
