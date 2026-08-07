//! Textures, keyed by the path a scene file names them with.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use voltra_render::wgpu::{Device, Queue};
use voltra_render::{Filter, Texture};

use crate::error::AssetError;
use crate::handle::Handle;
use crate::path::AssetPath;
use crate::placeholder;
use crate::store::Assets;

/// Loads textures from an asset root and hands out shared handles to them.
///
/// Two entities naming one PNG get one handle and therefore one GPU texture.
/// That is the whole point of the type.
pub struct Textures {
    root: PathBuf,
    store: Assets<Texture>,
    by_path: HashMap<AssetPath, Handle<Texture>>,
    placeholder: Handle<Texture>,
}

impl Textures {
    /// Builds a store rooted at `root`, with the placeholder already in it.
    pub fn new(device: &Device, queue: &Queue, root: impl Into<PathBuf>) -> Self {
        let mut store = Assets::new();
        let texture = Texture::from_rgba8(
            device,
            queue,
            "missing-texture",
            &placeholder::rgba(),
            placeholder::SIZE,
            placeholder::SIZE,
            // Nearest so the checks stay hard-edged. Filtering them into a
            // magenta smear makes the failure look like a design choice.
            Filter::Nearest,
        )
        .expect("the placeholder's pixel count matches its declared size");

        let placeholder = store.insert(texture);

        Self {
            root: root.into(),
            store,
            by_path: HashMap::new(),
            placeholder,
        }
    }

    /// The handle for `path`, loading it if this is the first time.
    ///
    /// Infallible on purpose: a scene naming a texture that will not load must
    /// still open and still draw. A failure logs once and returns the
    /// placeholder, and that answer is cached like any other — otherwise a
    /// broken path re-reads the disk and warns on every frame it is drawn.
    pub fn load(&mut self, device: &Device, queue: &Queue, path: &AssetPath) -> Handle<Texture> {
        if let Some(handle) = self.by_path.get(path) {
            return *handle;
        }

        let handle = match self.read(device, queue, path) {
            Ok(texture) => self.store.insert(texture),
            Err(e) => {
                log::warn!("{e}; drawing the missing-texture checker instead");
                self.placeholder
            }
        };

        self.by_path.insert(path.clone(), handle);
        handle
    }

    /// The texture `handle` names.
    ///
    /// Every handle this type hands out is valid: the placeholder is inserted
    /// at construction and nothing here ever removes from the store. Only a
    /// handle forged from a different store can reach the `expect`.
    pub fn get(&self, handle: Handle<Texture>) -> &Texture {
        self.store
            .get(handle)
            .expect("Textures never removes, so every handle it issued resolves")
    }

    /// The checker drawn in place of a texture that would not load.
    pub fn placeholder(&self) -> Handle<Texture> {
        self.placeholder
    }

    /// The directory every [`AssetPath`] is resolved against.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// How many textures are stored, the placeholder included.
    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    /// Reads and uploads one file. The only place this type touches the disk.
    fn read(
        &self,
        device: &Device,
        queue: &Queue,
        path: &AssetPath,
    ) -> Result<Texture, AssetError> {
        // Safe to join because `AssetPath` has already refused anything that
        // could climb out of the root. That check lives in the constructor
        // precisely so it cannot be forgotten here.
        let full = self.root.join(path.as_str());

        let bytes = std::fs::read(&full).map_err(|source| AssetError::Read {
            path: full.clone(),
            source,
        })?;

        Texture::from_png(device, queue, path.as_str(), &bytes, Filter::Linear)
            .map_err(|source| AssetError::Decode { path: full, source })
    }
}
