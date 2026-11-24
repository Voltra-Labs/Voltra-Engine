//! The language a game is in, and its strings: one table per language,
//! `locale/<language>.ron`, keys to strings.

mod table;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use voltra_assets::{AssetPath, Fonts};
use voltra_ecs::World;

pub use table::{StringTable, TableError};

use crate::Text;

/// The folder under the asset root the string tables are in.
pub const LOCALE_DIR: &str = "locale";

/// The current language and its string table.
///
/// Godot's `TranslationServer` with one CSV column per file, Unity's
/// string tables: a [`Text`] naming a [`key`](Text::key) shows the current
/// table's string for it, and a game asks for strings it builds itself with
/// [`text`](Self::text). A key the table lacks — or a language with no table
/// at all — shows the key itself, logged once per key and language: a
/// missing translation shows where it is missing rather than as nothing.
///
/// Tables are read through the asset funnel, so a web build reads them from
/// its bundle, and a saved table is read again while the game runs.
#[derive(Debug, Default)]
pub struct Locale {
    root: Option<PathBuf>,
    language: String,
    strings: HashMap<String, String>,
    /// The current table's fallback fonts.
    fonts: Vec<AssetPath>,
    /// Keys already warned about in this language.
    missing: RefCell<HashSet<String>>,
}

impl Locale {
    /// A locale reading its tables under `root`, in `language`.
    pub fn open(root: impl Into<PathBuf>, language: &str) -> Self {
        let mut locale = Self {
            root: Some(root.into()),
            ..Self::default()
        };
        locale.set_language(language);
        locale
    }

    /// The current language, as its table is named: `en`, `es`, `pt-BR`.
    pub fn language(&self) -> &str {
        &self.language
    }

    /// Switches to `language` and reads its table. A language with no table
    /// is still switched to — its texts show their keys — and said so;
    /// `false` then, or for a name no file can have.
    pub fn set_language(&mut self, language: &str) -> bool {
        self.language = language.to_owned();
        self.strings.clear();
        self.fonts.clear();
        self.missing.borrow_mut().clear();
        self.read()
    }

    /// Reads the current language's table again if `path` is it; whether it
    /// was. A table that no longer reads keeps the strings already read.
    pub fn reload(&mut self, path: &AssetPath) -> bool {
        if table_path(&self.language).as_ref() != Some(path) {
            return false;
        }
        self.missing.borrow_mut().clear();
        self.read();
        true
    }

    fn read(&mut self) -> bool {
        let Some(path) = table_path(&self.language) else {
            log::warn!(
                "{:?} is not a language a table can be named for",
                self.language
            );
            return false;
        };
        let Some(root) = &self.root else {
            return false;
        };
        match StringTable::load(&root.join(path.as_str())) {
            Ok(table) => {
                log::info!(
                    "strings for {} ({} keys)",
                    self.language,
                    table.strings.len()
                );
                self.strings = table.strings;
                self.fonts = table
                    .fonts
                    .iter()
                    .filter_map(|raw| match AssetPath::new(raw) {
                        Ok(path) => Some(path),
                        Err(e) => {
                            log::warn!("{} names the font {raw:?}: {e}", self.language);
                            None
                        }
                    })
                    .collect();
                true
            }
            Err(error) => {
                log::warn!(
                    "no strings for {}: {error}; texts show their keys",
                    self.language
                );
                false
            }
        }
    }

    /// The current language's string for `key`, or the key itself when the
    /// table lacks it — logged the first time.
    pub fn text<'a>(&'a self, key: &'a str) -> &'a str {
        if let Some(string) = self.strings.get(key) {
            return string;
        }
        if self.missing.borrow_mut().insert(key.to_owned()) {
            log::warn!("no {:?} string for the key {key:?}", self.language);
        }
        key
    }

    /// The fonts the current language's table names, tried after a text's
    /// own font for the glyphs it lacks.
    pub fn fonts(&self) -> &[AssetPath] {
        &self.fonts
    }

    /// Makes the current language's [`fonts`](Self::fonts) the fallback
    /// chain of `fonts`. Every frame, after [`localize`](Self::localize):
    /// unchanged, it costs a comparison.
    pub fn apply_fonts(&self, fonts: &mut Fonts) {
        fonts.set_fallbacks(&self.fonts);
    }

    /// Writes into every keyed [`Text`] its string in the current language.
    /// Every frame: a text spawned, a key edited, a language switched or a
    /// table reloaded is the next frame's content.
    pub fn localize(&self, world: &mut World) {
        for (_, text) in world.query_mut::<Text>() {
            let Some(key) = text.key.as_deref() else {
                continue;
            };
            let string = self.text(key);
            if text.content != string {
                text.content = string.to_owned();
            }
        }
    }
}

/// Where `language`'s table is, or `None` for a name that is no file name:
/// letters, digits, `-` and `_` only.
pub fn table_path(language: &str) -> Option<AssetPath> {
    let plain = !language.is_empty()
        && language
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    plain
        .then(|| AssetPath::new(&format!("{LOCALE_DIR}/{language}.ron")).ok())
        .flatten()
}

#[cfg(test)]
mod tests;
