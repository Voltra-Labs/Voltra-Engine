//! Prefabs, keyed by the path a game or a scene names them with.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use voltra_assets::{AssetPath, Assets, Handle};

use super::Prefab;
use crate::format::{SceneError, SceneFile};

/// Reads prefabs from an asset root and hands out shared handles to them.
///
/// The same shape as `Textures`, `Atlases` and `Clips` — read once per path,
/// a stable handle, an empty answer for what will not read — and here rather
/// than beside them in `voltra-assets` because reading one means parsing a
/// [`SceneFile`], which is this crate's format and depends on this crate's
/// components.
#[derive(Debug)]
pub struct Prefabs {
    root: PathBuf,
    store: Assets<Prefab>,
    by_path: HashMap<AssetPath, Handle<Prefab>>,
    empty: Handle<Prefab>,
}

impl Prefabs {
    /// A store rooted at `root`, with the empty prefab already in it.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let mut store = Assets::new();
        let empty = store.insert(Prefab::empty());
        Self {
            root: root.into(),
            store,
            by_path: HashMap::new(),
            empty,
        }
    }

    /// The handle for `path`, reading it if this is the first time.
    ///
    /// Infallible, the way every other store's load is: a game naming a
    /// prefab that will not read must keep running. A failure logs once and
    /// answers a prefab that spawns nothing — not a placeholder entity, which
    /// would be content nobody authored appearing in someone's world. Cached,
    /// so a broken file is not re-read on every spawn of it.
    pub fn load(&mut self, path: &AssetPath) -> Handle<Prefab> {
        if let Some(handle) = self.by_path.get(path) {
            return *handle;
        }

        let handle = match self.read(path) {
            Ok(prefab) => self.store.insert(prefab),
            Err(e) => {
                log::warn!("prefab {}: {e}; it will spawn nothing", path.as_str());
                // Its own slot, so repairing this file is a reload of this
                // handle and not of every other broken path's.
                self.store.insert(Prefab::empty())
            }
        };

        self.by_path.insert(path.clone(), handle);
        handle
    }

    /// Re-reads `path` under the handle it already has. Returns whether
    /// anything changed.
    ///
    /// Instances already spawned are untouched — they are plain entities and
    /// have forgotten the file — and the next spawn gets the edit. A path
    /// nothing loaded is ignored, and a file that will not read keeps the
    /// previous prefab, for the reason every other store gives: an editor
    /// mid-save leaves it truncated.
    pub fn reload(&mut self, path: &AssetPath) -> bool {
        let Some(&handle) = self.by_path.get(path) else {
            return false;
        };

        let prefab = match self.read(path) {
            Ok(prefab) => prefab,
            Err(e) => {
                log::warn!(
                    "prefab {}: {e}; keeping the previously loaded one",
                    path.as_str()
                );
                return false;
            }
        };

        let slot = self
            .store
            .get_mut(handle)
            .expect("Prefabs never removes, so every handle it issued resolves");
        *slot = prefab;
        log::info!("reloaded {}", path.as_str());
        true
    }

    /// The handle `path` is cached to, if it has ever been loaded.
    pub fn by_path_handle(&self, path: &AssetPath) -> Option<Handle<Prefab>> {
        self.by_path.get(path).copied()
    }

    /// The prefab `handle` names.
    ///
    /// Every handle this type hands out resolves: the empty one is inserted at
    /// construction and nothing here ever removes. Only a handle forged
    /// elsewhere can reach the `expect`.
    pub fn get(&self, handle: Handle<Prefab>) -> &Prefab {
        self.try_get(handle)
            .expect("Prefabs never removes, so every handle it issued resolves")
    }

    /// The prefab `handle` names, or `None` for a handle this store never
    /// issued.
    pub fn try_get(&self, handle: Handle<Prefab>) -> Option<&Prefab> {
        self.store.get(handle)
    }

    /// The prefab that spawns nothing.
    pub fn empty(&self) -> Handle<Prefab> {
        self.empty
    }

    /// The directory every [`AssetPath`] is resolved against.
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    /// Reads, parses and checks one file. The only place this type touches
    /// the disk.
    fn read(&self, path: &AssetPath) -> Result<Prefab, SceneError> {
        let text = voltra_assets::fs::read_to_string(&self.root.join(path.as_str()))?;
        let file: SceneFile = ron::from_str(&text).map_err(SceneError::Parse)?;
        Ok(Prefab::new(file)?.with_source(path.clone()))
    }
}
