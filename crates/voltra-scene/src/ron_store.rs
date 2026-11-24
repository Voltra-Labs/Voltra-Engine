//! A store of assets that are one RON file each: animation clips, state
//! machines.
//!
//! The shape every store here has — read once per path, a stable handle,
//! an empty answer for what will not read, a reload under the same handle,
//! an atomic save — written once for any type that is a whole file, rather
//! than once per type. Prefabs keep their own store: a scene file is
//! versioned and spawned, not merely read.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;
use voltra_assets::{AssetPath, Assets, Handle};

/// An asset that is a whole RON file.
pub trait RonAsset: Serialize + DeserializeOwned + Default + 'static {
    /// What a log line calls one: "animation", "state machine".
    const KIND: &'static str;

    /// The value as its users can trust it, from what the file said —
    /// sorted, with what cannot be used dropped and warned about, naming
    /// `source`. As read, by default.
    fn checked(self, _source: &str) -> Self {
        self
    }
}

/// Why a file could not be read or written.
#[derive(Debug)]
pub enum RonAssetError {
    /// The file could not be read or written.
    Io(std::io::Error),
    /// The text is not valid RON for the type.
    Parse(ron::error::SpannedError),
    /// The value could not be turned into RON.
    Serialize(ron::Error),
}

impl fmt::Display for RonAssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "i/o failed: {e}"),
            Self::Parse(e) => write!(f, "not valid RON: {e}"),
            Self::Serialize(e) => write!(f, "could not be written: {e}"),
        }
    }
}

impl std::error::Error for RonAssetError {}

/// Reads `T`s from an asset root and hands out shared handles to them.
#[derive(Debug)]
pub struct RonStore<T: RonAsset> {
    root: PathBuf,
    store: Assets<T>,
    by_path: HashMap<AssetPath, Handle<T>>,
}

impl<T: RonAsset> RonStore<T> {
    /// A store reading from under `root`, with nothing loaded yet.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            store: Assets::new(),
            by_path: HashMap::new(),
        }
    }

    /// The handle for `path`, reading it the first time.
    ///
    /// Infallible: a file that will not read logs once and is the empty
    /// value, in a slot of its own, so repairing the file is a reload of
    /// that handle.
    pub fn load(&mut self, path: &AssetPath) -> Handle<T> {
        if let Some(&handle) = self.by_path.get(path) {
            return handle;
        }
        let value = self.read(path).unwrap_or_else(|e| {
            log::warn!(
                "{} {}: {e}; it is empty until fixed",
                T::KIND,
                path.as_str()
            );
            T::default()
        });
        let handle = self.store.insert(value);
        self.by_path.insert(path.clone(), handle);
        handle
    }

    /// Re-reads `path` under its handle. Answers whether it was one this
    /// store had loaded and the new text read; a file that will not —
    /// truncated mid-save — keeps the value it had.
    pub fn reload(&mut self, path: &AssetPath) -> bool {
        let Some(&handle) = self.by_path.get(path) else {
            return false;
        };
        let value = match self.read(path) {
            Ok(value) => value,
            Err(e) => {
                log::warn!("{} {}: {e}; keeping the one loaded", T::KIND, path.as_str());
                return false;
            }
        };
        *self.slot(handle) = value;
        log::info!("reloaded {}", path.as_str());
        true
    }

    /// The value `handle` names.
    pub fn get(&self, handle: Handle<T>) -> &T {
        self.store
            .get(handle)
            .expect("a RonStore never removes, so every handle it issued resolves")
    }

    /// The value `handle` names, to edit in place — seen by every user of
    /// it at once, and written with [`save`](Self::save).
    pub fn get_mut(&mut self, handle: Handle<T>) -> &mut T {
        self.slot(handle)
    }

    /// The handle `path` is cached to, if it has been loaded.
    pub fn by_path_handle(&self, path: &AssetPath) -> Option<Handle<T>> {
        self.by_path.get(path).copied()
    }

    /// Writes the value `path` holds to its file, pretty-printed, through a
    /// temporary file renamed over it — the watcher sees one change, and a
    /// crash mid-write leaves the old file. A path never loaded is loaded
    /// first (and so written as it reads).
    pub fn save(&mut self, path: &AssetPath) -> Result<(), RonAssetError> {
        let handle = self.load(path);
        let text = ron::ser::to_string_pretty(self.get(handle), ron::ser::PrettyConfig::default())
            .map_err(RonAssetError::Serialize)?;
        let file = self.root.join(path.as_str());
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).map_err(RonAssetError::Io)?;
        }
        let tmp = file.with_extension("ron.tmp");
        std::fs::write(&tmp, text).map_err(RonAssetError::Io)?;
        std::fs::rename(&tmp, &file).map_err(RonAssetError::Io)
    }

    /// The directory every path is resolved against.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// How many assets are loaded.
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Whether no asset is loaded.
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    fn slot(&mut self, handle: Handle<T>) -> &mut T {
        self.store
            .get_mut(handle)
            .expect("a RonStore never removes, so every handle it issued resolves")
    }

    /// The only place this type reads the disk.
    fn read(&self, path: &AssetPath) -> Result<T, RonAssetError> {
        let text = voltra_assets::fs::read_to_string(&self.root.join(path.as_str()))
            .map_err(RonAssetError::Io)?;
        let value: T = ron::from_str(&text).map_err(RonAssetError::Parse)?;
        Ok(value.checked(path.as_str()))
    }
}
