//! One language's strings, as its file holds them.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

/// The format a table file is in. Version 2 added [`fonts`](StringTable::fonts);
/// a version 1 file reads as one naming none.
pub const TABLE_VERSION: u32 = 2;

/// `locale/<language>.ron`:
///
/// ```ron
/// (
///     version: 2,
///     fonts: ["fonts/NotoSansArabic-Regular.ttf"],
///     strings: {
///         "menu.play": "العب",
///         "dialogue.intro": "مرحبا أيها المسافر.",
///     },
/// )
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StringTable {
    /// The format version, at most [`TABLE_VERSION`].
    pub version: u32,
    /// The fonts a text in this language takes the glyphs its own font
    /// lacks from, tried in order before the default font: a game set in
    /// a Latin face still shows Arabic, Devanagari or Hebrew strings.
    /// Plain paths relative to the asset root, as a translator writes them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<String>,
    #[serde(default)]
    /// The text for each key, such as `menu.play`.
    pub strings: HashMap<String, String>,
}

/// Why a table did not read.
#[derive(Debug)]
pub enum TableError {
    /// The file could not be read.
    Io(std::io::Error),
    /// The file is not valid RON for a table.
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

    /// Reads a table from RON `text`.
    pub fn parse(text: &str) -> Result<Self, TableError> {
        let table: Self = ron::from_str(text).map_err(TableError::Parse)?;
        if table.version > TABLE_VERSION {
            return Err(TableError::Version(table.version));
        }
        Ok(table)
    }
}
