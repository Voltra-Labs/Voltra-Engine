//! Clips, keyed by the path a player names them with.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use voltra_assets::{AssetPath, Assets, Handle};

use super::clip::AnimationClip;

/// Why a clip file could not be read or written.
#[derive(Debug)]
pub enum ClipError {
    Io(std::io::Error),
    Parse(ron::error::SpannedError),
    Serialize(ron::Error),
}

impl fmt::Display for ClipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "animation clip i/o failed: {e}"),
            Self::Parse(e) => write!(f, "animation clip is not valid RON: {e}"),
            Self::Serialize(e) => write!(f, "could not write the animation clip: {e}"),
        }
    }
}

impl std::error::Error for ClipError {}

/// Reads clips from an asset root and hands out shared handles to them.
///
/// The shape of every other store — read once per path, a stable handle,
/// an empty answer for what will not read, a reload under the same handle —
/// and here rather than in `voltra-assets` because a clip names this
/// crate's properties.
#[derive(Debug)]
pub struct AnimationClips {
    root: PathBuf,
    store: Assets<AnimationClip>,
    by_path: HashMap<AssetPath, Handle<AnimationClip>>,
}

impl AnimationClips {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            store: Assets::new(),
            by_path: HashMap::new(),
        }
    }

    /// The handle for `path`, reading it the first time.
    ///
    /// Infallible: a clip that will not read logs once and is an empty clip
    /// — a player of it holds still — in a slot of its own, so repairing the
    /// file is a reload of that handle.
    pub fn load(&mut self, path: &AssetPath) -> Handle<AnimationClip> {
        if let Some(&handle) = self.by_path.get(path) {
            return handle;
        }
        let clip = self.read(path).unwrap_or_else(|e| {
            log::warn!("animation {}: {e}; it will animate nothing", path.as_str());
            AnimationClip::default()
        });
        let handle = self.store.insert(clip);
        self.by_path.insert(path.clone(), handle);
        handle
    }

    /// Re-reads `path` under its handle. Answers whether it was a clip this
    /// store had loaded and the new text read; a file that will not —
    /// truncated mid-save — keeps the clip it had.
    pub fn reload(&mut self, path: &AssetPath) -> bool {
        let Some(&handle) = self.by_path.get(path) else {
            return false;
        };
        let clip = match self.read(path) {
            Ok(clip) => clip,
            Err(e) => {
                log::warn!("animation {}: {e}; keeping the one loaded", path.as_str());
                return false;
            }
        };
        *self.slot(handle) = clip;
        log::info!("reloaded {}", path.as_str());
        true
    }

    /// The clip `handle` names.
    pub fn get(&self, handle: Handle<AnimationClip>) -> &AnimationClip {
        self.store
            .get(handle)
            .expect("AnimationClips never removes, so every handle it issued resolves")
    }

    /// The clip `handle` names, to edit in place — an editor's keys, seen
    /// by every player of it at once, and saved with [`save`](Self::save).
    pub fn get_mut(&mut self, handle: Handle<AnimationClip>) -> &mut AnimationClip {
        self.slot(handle)
    }

    /// The handle `path` is cached to, if it has been loaded.
    pub fn by_path_handle(&self, path: &AssetPath) -> Option<Handle<AnimationClip>> {
        self.by_path.get(path).copied()
    }

    /// Writes the clip `path` holds to its file, pretty-printed, through a
    /// temporary file renamed over it — the watcher sees one change, and a
    /// crash mid-write leaves the old file. A path never loaded is loaded
    /// first (and so written as it reads).
    pub fn save(&mut self, path: &AssetPath) -> Result<(), ClipError> {
        let handle = self.load(path);
        let text = ron::ser::to_string_pretty(self.get(handle), ron::ser::PrettyConfig::default())
            .map_err(ClipError::Serialize)?;
        let file = self.root.join(path.as_str());
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).map_err(ClipError::Io)?;
        }
        let tmp = file.with_extension("ron.tmp");
        std::fs::write(&tmp, text).map_err(ClipError::Io)?;
        std::fs::rename(&tmp, &file).map_err(ClipError::Io)
    }

    /// The directory every path is resolved against.
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    fn slot(&mut self, handle: Handle<AnimationClip>) -> &mut AnimationClip {
        self.store
            .get_mut(handle)
            .expect("AnimationClips never removes, so every handle it issued resolves")
    }

    /// The only place this type reads the disk.
    fn read(&self, path: &AssetPath) -> Result<AnimationClip, ClipError> {
        let text = std::fs::read_to_string(self.root.join(path.as_str())).map_err(ClipError::Io)?;
        let clip: AnimationClip = ron::from_str(&text).map_err(ClipError::Parse)?;
        Ok(clip.checked(path.as_str()))
    }
}
