//! Prefabs, keyed by the path a game or a scene names them with.

mod read;
mod reload;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use voltra_assets::{AssetPath, Assets, Handle};

use super::cycle::nested_counted;
use super::resolve::{resolve, ResolveProblem, MAX_DEPTH, MAX_LINKS, MAX_SOURCES};
use super::Prefab;
use crate::format::{ComponentRegistry, SceneFile};

/// The largest prefab file the store reads: a prefab is an asset read from
/// disk, and one this size is a mistake or an attack, not a template.
pub const MAX_PREFAB_BYTES: u64 = 16 * 1024 * 1024;

/// Reads prefabs from an asset root and hands out shared handles to them.
///
/// The same shape as `Textures`, `Atlases` and `Clips` — read once per path,
/// a stable handle, an empty answer for what will not read — and here rather
/// than beside them in `voltra-assets` because reading one means parsing a
/// [`SceneFile`], which is this crate's format and depends on this crate's
/// components.
///
/// **A prefab is read as what it spawns**: every instance stored in it —
/// a variant's base, a prefab nested inside — is read in turn and brought
/// in step with it, through the store's [`ComponentRegistry`], so the
/// store holds each file once and every file holding an instance of it
/// follows it. See `resolve.rs`.
#[derive(Debug)]
pub struct Prefabs {
    root: PathBuf,
    store: Assets<Prefab>,
    by_path: HashMap<AssetPath, Handle<Prefab>>,
    empty: Handle<Prefab>,
    /// What stored instances are compared and copied through.
    registry: ComponentRegistry,
    /// Prefabs being read again because a file they hold an instance of
    /// changed, by path, with the handle each keeps.
    stale: HashMap<AssetPath, Handle<Prefab>>,
}

impl Prefabs {
    /// A store rooted at `root`, with the empty prefab already in it,
    /// reading stored instances through the engine's own components. A
    /// game with components of its own gives its registry with
    /// [`with_registry`](Self::with_registry), or a variant of one of its
    /// prefabs would not follow those.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::with_registry(root, ComponentRegistry::with_defaults())
    }

    /// A store rooted at `root` reading stored instances through
    /// `registry`.
    pub fn with_registry(root: impl Into<PathBuf>, registry: ComponentRegistry) -> Self {
        let mut store = Assets::new();
        let empty = store.insert(Prefab::empty());
        Self {
            root: root.into(),
            store,
            by_path: HashMap::new(),
            empty,
            registry,
            stale: HashMap::new(),
        }
    }

    /// The handle for `path`, reading it if this is the first time.
    ///
    /// Infallible, the way every other store's load is: a game naming a
    /// prefab that will not read must keep running. A failure logs once and
    /// answers a prefab that spawns nothing — not a placeholder entity, which
    /// would be content nobody authored appearing in someone's world. Cached,
    /// so a broken file is not re-read on every spawn of it.
    ///
    /// The prefabs it holds instances of are read too. One that leads back
    /// to it, nests deeper than [`MAX_DEPTH`] or will not follow is logged
    /// and left as the file has it ([`Prefab::problems`]).
    pub fn load(&mut self, path: &AssetPath) -> Handle<Prefab> {
        self.load_at(path, &mut Vec::new())
    }

    /// [`load`](Self::load) from inside the reading of `stack`, the
    /// prefabs whose stored instances led here, outermost first.
    fn load_at(&mut self, path: &AssetPath, stack: &mut Vec<AssetPath>) -> Handle<Prefab> {
        let mut kept = self.stale.remove(path);
        if let Some(&handle) = self.by_path.get(path) {
            let prefab = self.get(handle);
            // Read as a scene's file, and now instanced: followed now.
            if prefab.resolved || prefab.file.is_none() {
                return handle;
            }
            self.by_path.remove(path);
            kept = Some(handle);
        }
        let stored = match kept {
            Some(handle) => self.get(handle).stored_file().cloned().ok_or(None),
            None => self.read(path, Some(MAX_PREFAB_BYTES)).map_err(Some),
        };
        let prefab = match stored {
            Ok(file) => self.resolved(path, file, stack),
            Err(error) => {
                if let Some(e) = error {
                    log::warn!("prefab {}: {e}; it will spawn nothing", path.as_str());
                }
                Prefab::empty()
            }
        };
        self.put(path, prefab, kept)
    }

    /// The handle for the scene at `path`, read as [`load`](Self::load)
    /// reads a prefab if this is the first time — but as a level is: the
    /// instances it holds are not followed (a sync after the load brings
    /// them in step, in the world) and no prefab limit applies, as none
    /// applied to a scene before. One copy is kept. If the same file is
    /// later [`load`](Self::load)ed as a prefab, it is followed then, under
    /// the same handle.
    pub fn load_scene(&mut self, path: &AssetPath) -> Handle<Prefab> {
        if let Some(&handle) = self.by_path.get(path) {
            return handle;
        }
        let prefab = match self.read(path, None) {
            Ok(file) => Prefab {
                file: Some(file),
                source: Some(path.clone()),
                ..Prefab::default()
            },
            Err(e) => {
                log::warn!("scene {}: {e}; it will spawn nothing", path.as_str());
                Prefab::empty()
            }
        };
        self.put(path, prefab, None)
    }

    /// `stored`, the file at `path`, with the prefabs its instances name
    /// read and its instances brought in step with them.
    fn resolved(
        &mut self,
        path: &AssetPath,
        stored: SceneFile,
        stack: &mut Vec<AssetPath>,
    ) -> Prefab {
        let mut problems = Vec::new();
        let mut handles = Vec::new();
        // Every prefab a link names, not only the outermost instances': an
        // instance added inside one is synced with its own file too.
        let (sources, links) = nested_counted(&stored);
        if links > MAX_LINKS || sources.len() > MAX_SOURCES {
            let problem = ResolveProblem::TooManyLinks {
                links,
                sources: sources.len(),
            };
            log::error!(
                "prefab {}: {problem}; left as its file has it",
                path.as_str()
            );
            return Prefab {
                file: Some(stored),
                stored: None,
                source: Some(path.clone()),
                problems: vec![problem],
                resolved: true,
            };
        }
        stack.push(path.clone());
        for source in sources {
            if let Some(at) = stack.iter().position(|p| *p == source) {
                let mut chain = stack[at..].to_vec();
                chain.push(source);
                problems.push(ResolveProblem::Cycle { chain });
            } else if stack.len() >= MAX_DEPTH {
                problems.push(ResolveProblem::TooDeep { source });
            } else {
                let handle = self.load_at(&source, stack);
                handles.push((source, handle));
            }
        }
        stack.pop();

        let followed: HashMap<AssetPath, &Prefab> = handles
            .iter()
            .map(|(source, handle)| (source.clone(), self.get(*handle)))
            .filter(|(_, prefab)| prefab.source.is_some())
            .collect();
        let (file, stored, mut more) = match resolve(&stored, &self.registry, &followed) {
            Ok((Some(file), more)) => (file, Some(stored), more),
            Ok((None, more)) => (stored, None, more),
            Err(error) => {
                let problem = ResolveProblem::Failed {
                    source: path.clone(),
                    error: error.to_string(),
                };
                (stored, None, vec![problem])
            }
        };
        problems.append(&mut more);
        // Once per file, however many instances in it went wrong.
        if let Some(first) = problems.first() {
            let more = match problems.len() {
                1 => String::new(),
                n => format!(" (and {} more)", n - 1),
            };
            log::error!(
                "prefab {}: {first}{more}; left as its file has it",
                path.as_str()
            );
        }
        Prefab {
            file: Some(file),
            stored,
            source: Some(path.clone()),
            problems,
            resolved: true,
        }
    }

    /// Caches `prefab` under `path`, in the slot `kept` when it has one.
    fn put(
        &mut self,
        path: &AssetPath,
        prefab: Prefab,
        kept: Option<Handle<Prefab>>,
    ) -> Handle<Prefab> {
        let handle = match kept {
            Some(handle) => {
                *self
                    .store
                    .get_mut(handle)
                    .expect("Prefabs never removes, so every handle it issued resolves") = prefab;
                handle
            }
            // Its own slot even when empty, so repairing this file is a
            // reload of this handle and not of every other broken path's.
            None => self.store.insert(prefab),
        };
        self.by_path.insert(path.clone(), handle);
        handle
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

    /// The registry stored instances are read through.
    pub fn registry(&self) -> &ComponentRegistry {
        &self.registry
    }

    /// How many prefabs are loaded.
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Whether no prefab is loaded.
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}
