//! Textures, keyed by the path a scene file names them with.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use voltra_render::wgpu::{BindGroup, BindGroupLayout, Device, Queue};
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
    /// The layout every bind group below is created against. Cloning is
    /// cheap — `wgpu::BindGroupLayout` is reference-counted internally — and
    /// keeping it here is what lets `load` create a bind group for a texture
    /// loaded long after construction without the caller re-supplying the
    /// layout the pipeline was built with.
    layout: BindGroupLayout,
    /// One bind group per handle in `store`, inserted the moment the texture
    /// is. A handle that resolves in `store` but not here would be a bug at
    /// load time rather than the render-time validation panic it would
    /// otherwise surface as.
    bind_groups: HashMap<Handle<Texture>, BindGroup>,
}

impl Textures {
    /// Builds a store rooted at `root`, with the placeholder already in it.
    ///
    /// `layout` must be the same object the sprite pipeline was built with —
    /// a bind group is only valid against the layout it was created from.
    pub fn new(
        device: &Device,
        queue: &Queue,
        layout: &BindGroupLayout,
        root: impl Into<PathBuf>,
    ) -> Self {
        let mut store = Assets::new();
        let texture = placeholder_texture(device, queue);
        let bind_group = texture.create_bind_group(device, layout);
        let placeholder = store.insert(texture);

        let mut bind_groups = HashMap::new();
        bind_groups.insert(placeholder, bind_group);

        Self {
            root: root.into(),
            store,
            by_path: HashMap::new(),
            placeholder,
            layout: layout.clone(),
            bind_groups,
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
            Ok(texture) => {
                let bind_group = texture.create_bind_group(device, &self.layout);
                let handle = self.store.insert(texture);
                self.bind_groups.insert(handle, bind_group);
                handle
            }
            Err(e) => {
                log::warn!("{e}; drawing the missing-texture checker instead");
                self.insert_broken(device, queue)
            }
        };

        self.by_path.insert(path.clone(), handle);
        handle
    }

    /// A private texture holding the placeholder's pixels, under its own
    /// handle.
    ///
    /// Not the shared [`Self::placeholder`] handle. A sprite stores the handle
    /// it was given and nothing re-resolves it, so if every broken path shared
    /// one handle, repairing one of them would mean overwriting the texture all
    /// the others are also drawing — and hot reload could never fix a typo. Its
    /// own slot costs 256 bytes of pixels.
    fn insert_broken(&mut self, device: &Device, queue: &Queue) -> Handle<Texture> {
        let texture = placeholder_texture(device, queue);
        let bind_group = texture.create_bind_group(device, &self.layout);
        let handle = self.store.insert(texture);
        self.bind_groups.insert(handle, bind_group);
        handle
    }

    /// Re-reads `path` and swaps the new pixels in under the handle it already
    /// has. Returns whether anything changed.
    ///
    /// The handle is deliberately stable: every sprite in the world stores the
    /// one it was given, and the scene file stores the path. Issuing a new
    /// handle would mean walking the world to repoint them, and would make the
    /// scene dirty for a change nobody made to it.
    ///
    /// A path that was never loaded is ignored. The watch is on the whole asset
    /// root, so most events name files no scene has asked for; loading them
    /// here would upload every PNG in the project the first time one of them
    /// was touched.
    pub fn reload(&mut self, device: &Device, queue: &Queue, path: &AssetPath) -> bool {
        let Some(&handle) = self.by_path.get(path) else {
            return false;
        };

        let texture = match self.read(device, queue, path) {
            Ok(texture) => texture,
            Err(e) => {
                // The previous pixels stay on screen. An image editor's save
                // leaves the file truncated for a few milliseconds and the
                // debounce window does not always cover it, so degrading to the
                // checker here would flash magenta on every save.
                log::warn!("{e}; keeping the previously loaded texture");
                return false;
            }
        };

        let bind_group = texture.create_bind_group(device, &self.layout);
        let slot = self
            .store
            .get_mut(handle)
            .expect("Textures never removes, so every handle it issued resolves");
        *slot = texture;
        // The old bind group still names the old texture view, so replacing the
        // texture without replacing this swaps nothing the GPU can see.
        self.bind_groups.insert(handle, bind_group);

        log::info!("reloaded {}", path.as_str());
        true
    }

    /// The handle `path` is cached to, if it has ever been loaded.
    ///
    /// Distinct from [`Self::load`], which loads on a miss. This asks whether
    /// the cache already knows the path, which is the question hot reload has.
    pub fn by_path_handle(&self, path: &AssetPath) -> Option<Handle<Texture>> {
        self.by_path.get(path).copied()
    }

    /// The bind group `handle`'s texture and sampler were bound into.
    ///
    /// Every handle this type hands out has one: the placeholder gets its
    /// group at construction, and `load` creates one for every texture it
    /// inserts. Only a handle forged from a different store can reach the
    /// `expect`.
    pub fn bind_group(&self, handle: Handle<Texture>) -> &BindGroup {
        self.bind_groups
            .get(&handle)
            .expect("Textures inserts a bind group with every texture")
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

    /// Every handle a path currently resolves to, in no particular order.
    ///
    /// For a caller mirroring the store somewhere else — the editor's egui
    /// thumbnails — this is the set to mirror. A path whose file would not load
    /// resolves to the placeholder, so the same handle can appear twice; a
    /// caller keyed by handle collapses those, which is correct, because they
    /// really are one texture.
    pub fn loaded(&self) -> impl Iterator<Item = Handle<Texture>> + '_ {
        self.by_path.values().copied()
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

/// The magenta-and-black checker, uploaded.
///
/// A free function because both the store's seed and every failed path need
/// one, and the second of those runs on a `&mut self` that already exists.
fn placeholder_texture(device: &Device, queue: &Queue) -> Texture {
    Texture::from_rgba8(
        device,
        queue,
        "missing-texture",
        &placeholder::rgba(),
        placeholder::SIZE,
        placeholder::SIZE,
        // Nearest so the checks stay hard-edged. Filtering them into a magenta
        // smear makes the failure look like a design choice.
        Filter::Nearest,
    )
    .expect("the placeholder's pixel count matches its declared size")
}
