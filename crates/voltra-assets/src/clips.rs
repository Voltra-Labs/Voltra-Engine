//! Sounds, keyed by the path a scene file names them with.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use voltra_audio::{decode_bytes, AudioError, Clip};

use crate::error::AssetError;
use crate::handle::Handle;
use crate::path::AssetPath;
use crate::store::Assets;

/// Loads clips from an asset root and hands out shared handles to them.
///
/// The same shape as [`Textures`](crate::Textures) and
/// [`Atlases`](crate::Atlases) — load once per path, a stable handle, a
/// placeholder for what will not load — because three stores that behaved
/// differently would be three things to remember. It needs no GPU and no
/// audio device: decoding is arithmetic over a file, which is what keeps every
/// test of it headless.
///
/// Hot reload swaps the clip under a stable handle, and needs no message to
/// the audio thread because nothing is swapped *under* it: a [`Clip`] is an
/// immutable, shared buffer, and a voice holds its own reference. A voice
/// already playing finishes the samples it started with; the next play of the
/// handle gets the new ones. Restarting what is sounding — so a looping
/// ambience picks the edit up — is the caller's decision, because only it
/// knows which voices came from which handle.
#[derive(Debug)]
pub struct Clips {
    root: PathBuf,
    store: Assets<Clip>,
    by_path: HashMap<AssetPath, Handle<Clip>>,
    silent: Handle<Clip>,
}

impl Clips {
    /// A store rooted at `root`, with the silent clip already in it.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let mut store = Assets::new();
        let silent = store.insert(Clip::silent());
        Self {
            root: root.into(),
            store,
            by_path: HashMap::new(),
            silent,
        }
    }

    /// The handle for `path`, decoding it if this is the first time.
    ///
    /// Infallible, the way texture and atlas loading are: a scene naming a
    /// sound that will not decode must still open. A failure logs once and
    /// returns a silent clip — see [`Clip::silent`] on why silence and not a
    /// placeholder tone. The answer is cached, or a broken file would be
    /// re-read and re-warned every time something triggered it.
    pub fn load(&mut self, path: &AssetPath) -> Handle<Clip> {
        if let Some(handle) = self.by_path.get(path) {
            return *handle;
        }

        let handle = match decode_file(&self.root.join(path.as_str())) {
            Ok(clip) => self.store.insert(clip),
            Err(source) => {
                let error = AssetError::Audio {
                    path: self.root.join(path.as_str()),
                    source: Box::new(source),
                };
                log::warn!("{error}; it will play nothing");
                // Its own slot rather than the shared silent one, for the same
                // reason `Textures` gives each broken path its own placeholder:
                // repairing this file must not mean overwriting the clip every
                // other broken path is also using.
                self.store.insert(Clip::silent())
            }
        };

        self.by_path.insert(path.clone(), handle);
        handle
    }

    /// Decodes `path` again and swaps it in under the handle it already has.
    ///
    /// `false`, changing nothing, for a path nobody loaded — the watcher sees
    /// every sound in the tree, and decoding one no scene uses would be a
    /// waste — and for a file that will not decode. The second keeps the
    /// previous samples rather than going silent: an editor saving a file
    /// leaves it briefly truncated, and degrading to nothing on every save
    /// would be worse than hearing the old take for one more moment.
    pub fn reload(&mut self, path: &AssetPath) -> bool {
        let Some(&handle) = self.by_path.get(path) else {
            return false;
        };

        let clip = match decode_file(&self.root.join(path.as_str())) {
            Ok(clip) => clip,
            Err(source) => {
                let error = AssetError::Audio {
                    path: self.root.join(path.as_str()),
                    source: Box::new(source),
                };
                log::warn!("{error}; keeping the previously loaded clip");
                return false;
            }
        };

        let slot = self
            .store
            .get_mut(handle)
            .expect("Clips never removes, so every handle it issued resolves");
        *slot = clip;
        log::info!("reloaded {}", path.as_str());
        true
    }

    /// The handle `path` is cached to, if it has ever been loaded.
    pub fn by_path_handle(&self, path: &AssetPath) -> Option<Handle<Clip>> {
        self.by_path.get(path).copied()
    }

    /// The clip `handle` names.
    ///
    /// Every handle this type hands out resolves: the silent one is inserted
    /// at construction and nothing here ever removes. Only a handle forged
    /// elsewhere can reach the `expect`.
    pub fn get(&self, handle: Handle<Clip>) -> &Clip {
        self.try_get(handle)
            .expect("Clips never removes, so every handle it issued resolves")
    }

    /// The clip `handle` names, or `None` for a handle this store never
    /// issued — the same question [`get`](Self::get) answers, without the
    /// invariant, for a caller holding a handle from some other store.
    pub fn try_get(&self, handle: Handle<Clip>) -> Option<&Clip> {
        self.store.get(handle)
    }

    /// The clip with nothing in it, played by anything that would not decode.
    pub fn silent(&self) -> Handle<Clip> {
        self.silent
    }

    /// The directory every [`AssetPath`] is resolved against.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// How many clips are stored.
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Whether no clip is stored.
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}

/// Decodes the file at `full`, read through [`crate::fs`] so a mounted
/// pack serves it as the disk would.
fn decode_file(full: &Path) -> Result<Clip, AudioError> {
    let bytes = crate::fs::read(full).map_err(|source| AudioError::Read {
        path: full.to_owned(),
        source,
    })?;
    decode_bytes(bytes, full)
}

#[cfg(test)]
mod tests;
