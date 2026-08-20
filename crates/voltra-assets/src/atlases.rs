//! Slicings, keyed by the path a scene file names them with.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::atlas::{Atlas, AtlasFile};
use crate::error::AssetError;
use crate::handle::Handle;
use crate::path::AssetPath;
use crate::store::Assets;

/// Loads atlases from an asset root and hands out shared handles to them.
///
/// The same shape as [`Textures`](crate::Textures) — load once per path, a
/// stable handle, reload in place — and deliberately so: two stores that
/// behaved differently would mean two hot-reload stories. It needs no GPU,
/// because a slicing is arithmetic; that is also what keeps every test of it
/// headless.
#[derive(Debug)]
pub struct Atlases {
    root: PathBuf,
    store: Assets<Atlas>,
    by_path: HashMap<AssetPath, Handle<Atlas>>,
    empty: Handle<Atlas>,
}

impl Atlases {
    /// A store rooted at `root`, with the empty slicing already in it.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let mut store = Assets::new();
        let empty = store.insert(Atlas::default());
        Self {
            root: root.into(),
            store,
            by_path: HashMap::new(),
            empty,
        }
    }

    /// The handle for `path`, reading it if this is the first time.
    ///
    /// Infallible, as texture loading is: a scene naming an atlas that will not
    /// parse must still open. A failure logs once and returns an empty
    /// slicing, which makes every sprite using it draw the missing-frame
    /// placeholder — the same way a missing texture already draws the checker.
    /// The answer is cached, or a broken file would be re-read and re-warned
    /// on every frame it is drawn.
    pub fn load(&mut self, path: &AssetPath) -> Handle<Atlas> {
        if let Some(handle) = self.by_path.get(path) {
            return *handle;
        }

        let handle = match self.read(path) {
            Ok(atlas) => self.store.insert(atlas),
            Err(e) => {
                log::warn!("{e}; drawing no frame at all");
                // Its own slot rather than the shared empty one, so repairing
                // the file can fix this sprite without touching every other
                // broken atlas — the same reason `Textures` gives each broken
                // path its own placeholder.
                self.store.insert(Atlas::default())
            }
        };

        self.by_path.insert(path.clone(), handle);
        handle
    }

    /// Re-reads `path` under the handle it already has. Returns whether
    /// anything changed.
    ///
    /// A path nothing has loaded is ignored: the watch covers the whole root,
    /// and reading every atlas in the project the first time one of them is
    /// touched is not a reload.
    pub fn reload(&mut self, path: &AssetPath) -> bool {
        let Some(&handle) = self.by_path.get(path) else {
            return false;
        };

        let atlas = match self.read(path) {
            Ok(atlas) => atlas,
            Err(e) => {
                // The previous slicing stays. An editor saving the file leaves
                // it briefly truncated, and degrading to nothing would blank
                // every sprite using it on each save.
                log::warn!("{e}; keeping the previously loaded atlas");
                return false;
            }
        };

        let slot = self
            .store
            .get_mut(handle)
            .expect("Atlases never removes, so every handle it issued resolves");
        *slot = atlas;
        log::info!("reloaded {}", path.as_str());
        true
    }

    /// The handle `path` is cached to, if it has ever been loaded.
    pub fn by_path_handle(&self, path: &AssetPath) -> Option<Handle<Atlas>> {
        self.by_path.get(path).copied()
    }

    /// The slicing `handle` names.
    ///
    /// Every handle this type hands out resolves: the empty one is inserted at
    /// construction and nothing here ever removes. Only a handle forged
    /// elsewhere can reach the `expect`.
    pub fn get(&self, handle: Handle<Atlas>) -> &Atlas {
        self.store
            .get(handle)
            .expect("Atlases never removes, so every handle it issued resolves")
    }

    /// The slicing with no frames, drawn as no frame at all.
    pub fn empty(&self) -> Handle<Atlas> {
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

    /// Reads and parses one file. The only place this type touches the disk.
    fn read(&self, path: &AssetPath) -> Result<Atlas, AssetError> {
        let full = self.root.join(path.as_str());
        let text = std::fs::read_to_string(&full).map_err(|source| AssetError::Read {
            path: full.clone(),
            source,
        })?;
        let file: AtlasFile = ron::from_str(&text).map_err(|source| AssetError::Parse {
            path: full.clone(),
            source: Box::new(source),
        })?;
        file.into_atlas()
            .map_err(|source| AssetError::Atlas { path: full, source })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_render::glam::UVec2;
    use voltra_testkit::scratch_root;

    fn path(raw: &str) -> AssetPath {
        AssetPath::new(raw).expect("a valid asset path")
    }

    const GRID: &str = r#"(
        version: 1,
        grid: Some((cell: (16, 16), columns: 4, rows: 1)),
    )"#;

    #[test]
    fn a_file_becomes_a_slicing() {
        let root = scratch_root();
        std::fs::write(root.join("coin.atlas.ron"), GRID).expect("the fixture writes");
        let mut atlases = Atlases::new(&root);

        let handle = atlases.load(&path("coin.atlas.ron"));

        let atlas = atlases.get(handle);
        assert_eq!(atlas.len(), 4);
        assert_eq!(atlas.sheet(), UVec2::new(64, 16));
    }

    #[test]
    fn one_path_is_read_once() {
        let root = scratch_root();
        std::fs::write(root.join("coin.atlas.ron"), GRID).expect("the fixture writes");
        let mut atlases = Atlases::new(&root);

        let first = atlases.load(&path("coin.atlas.ron"));
        let before = atlases.len();
        let second = atlases.load(&path("coin.atlas.ron"));

        assert_eq!(first, second);
        assert_eq!(atlases.len(), before, "the second load stored nothing");
    }

    #[test]
    fn a_file_that_is_not_there_is_an_empty_slicing_rather_than_a_panic() {
        let root = scratch_root();
        let mut atlases = Atlases::new(&root);

        let handle = atlases.load(&path("nowhere.atlas.ron"));

        assert!(atlases.get(handle).is_empty());
        assert_ne!(
            handle,
            atlases.empty(),
            "and it gets its own slot to repair"
        );
    }

    #[test]
    fn a_file_that_will_not_parse_is_the_same_answer() {
        let root = scratch_root();
        std::fs::write(root.join("bad.atlas.ron"), "not ron at all").expect("the fixture writes");
        let mut atlases = Atlases::new(&root);

        let handle = atlases.load(&path("bad.atlas.ron"));

        assert!(atlases.get(handle).is_empty());
    }

    #[test]
    fn a_reload_swaps_the_frames_under_the_same_handle() {
        // The property every sprite in the world depends on: it stores the
        // handle it was given and nothing re-resolves it.
        let root = scratch_root();
        let file = root.join("coin.atlas.ron");
        std::fs::write(&file, GRID).expect("the fixture writes");
        let mut atlases = Atlases::new(&root);
        let handle = atlases.load(&path("coin.atlas.ron"));
        assert_eq!(atlases.get(handle).len(), 4);

        std::fs::write(
            &file,
            r#"(version: 1, grid: Some((cell: (8, 16), columns: 8, rows: 1)))"#,
        )
        .expect("the re-cut writes");
        let changed = atlases.reload(&path("coin.atlas.ron"));

        assert!(changed);
        assert_eq!(atlases.get(handle).len(), 8);
    }

    #[test]
    fn a_reload_that_fails_keeps_what_was_loaded() {
        let root = scratch_root();
        let file = root.join("coin.atlas.ron");
        std::fs::write(&file, GRID).expect("the fixture writes");
        let mut atlases = Atlases::new(&root);
        let handle = atlases.load(&path("coin.atlas.ron"));

        std::fs::write(&file, "half-written").expect("the truncation writes");

        assert!(!atlases.reload(&path("coin.atlas.ron")));
        assert_eq!(atlases.get(handle).len(), 4, "still on screen");
    }

    #[test]
    fn reloading_a_path_nobody_loaded_does_nothing() {
        let root = scratch_root();
        std::fs::write(root.join("coin.atlas.ron"), GRID).expect("the fixture writes");
        let mut atlases = Atlases::new(&root);

        assert!(!atlases.reload(&path("coin.atlas.ron")));
        assert_eq!(atlases.len(), 1, "only the empty slicing is stored");
    }
}
