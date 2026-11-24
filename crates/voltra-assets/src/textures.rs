//! Textures, keyed by the path a scene file names them with.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use voltra_render::texture::{decode_png, DecodedImage};
use voltra_render::wgpu::{BindGroup, BindGroupLayout, Device, Queue};
use voltra_render::{Filter, Texture};

use crate::error::AssetError;
use crate::handle::Handle;
use crate::path::AssetPath;
use crate::placeholder;
use crate::store::Assets;

mod generated;
mod preload;
mod render_texture;

pub use preload::DecodedTexture;
pub use render_texture::{
    is_render_texture, RenderTexture, RenderTextureFile, RENDER_TEXTURE_SUFFIX,
};

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
    /// The same textures bound through their linear view, for those used as
    /// data — a normal map — rather than colour. Kept beside `bind_groups`,
    /// entry for entry: one PNG can be both, and decoding it from sRGB as a
    /// normal map would bend every normal.
    raw_bind_groups: HashMap<Handle<Texture>, BindGroup>,
    /// Handles holding the checker because their file would not load, until
    /// a reload repairs them. Asked by a caller that uses a texture as data
    /// (a normal map), for which the checker's pixels would be nonsense.
    broken: HashSet<Handle<Texture>>,
    /// How every image is filtered when it does not land 1:1 on pixels: the
    /// project's choice, Godot's `default_texture_filter` — `Nearest` for
    /// pixel art.
    filter: Filter,
    /// The format a render texture is made in: the one the sprite pipeline
    /// draws into.
    target_format: voltra_render::wgpu::TextureFormat,
    /// The targets render textures are drawn into, by the handle sprites
    /// show them through.
    targets: HashMap<Handle<Texture>, RenderTexture>,
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
        let raw_bind_group = texture.create_raw_bind_group(device, layout);
        let placeholder = store.insert(texture);

        let mut bind_groups = HashMap::new();
        bind_groups.insert(placeholder, bind_group);
        let mut raw_bind_groups = HashMap::new();
        raw_bind_groups.insert(placeholder, raw_bind_group);

        Self {
            root: root.into(),
            store,
            by_path: HashMap::new(),
            placeholder,
            layout: layout.clone(),
            bind_groups,
            raw_bind_groups,
            broken: HashSet::new(),
            filter: Filter::Linear,
            target_format: voltra_render::wgpu::TextureFormat::Rgba8UnormSrgb,
            targets: HashMap::new(),
        }
    }

    /// Filters every image loaded from now on with `filter`. Set before
    /// anything loads, as a project setting is.
    pub fn set_filter(&mut self, filter: Filter) {
        self.filter = filter;
    }

    /// The filter images are loaded with.
    pub fn filter(&self) -> Filter {
        self.filter
    }

    /// Makes render textures in `format`: the format the renderer's sprite
    /// pipeline draws into, or a camera could not draw into them.
    pub fn set_target_format(&mut self, format: voltra_render::wgpu::TextureFormat) {
        self.target_format = format;
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

        if is_render_texture(path.as_str()) {
            return self.load_render_texture(device, queue, path);
        }
        let texture = self.read(device, queue, path);
        self.admit(device, queue, path, texture)
    }

    /// Caches `texture` under `path` — or, when it failed, a private
    /// placeholder, logged once — and returns its handle.
    pub(crate) fn admit(
        &mut self,
        device: &Device,
        queue: &Queue,
        path: &AssetPath,
        texture: Result<Texture, AssetError>,
    ) -> Handle<Texture> {
        let handle = match texture {
            Ok(texture) => self.insert(device, texture),
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
        let handle = self.insert(device, texture);
        self.broken.insert(handle);
        handle
    }

    /// Whether `handle` holds the checker in place of a file that would not
    /// load. A reload that succeeds repairs it.
    pub fn is_broken(&self, handle: Handle<Texture>) -> bool {
        self.broken.contains(&handle)
    }

    /// Stores `texture` under a new handle, with both its bind groups.
    pub(crate) fn insert(&mut self, device: &Device, texture: Texture) -> Handle<Texture> {
        let bind_group = texture.create_bind_group(device, &self.layout);
        let raw_bind_group = texture.create_raw_bind_group(device, &self.layout);
        let handle = self.store.insert(texture);
        self.bind_groups.insert(handle, bind_group);
        self.raw_bind_groups.insert(handle, raw_bind_group);
        handle
    }

    /// Puts `texture` under `handle` in place of what was there, with fresh
    /// bind groups: the old ones still name the old texture's views, so
    /// swapping the texture alone would change nothing the GPU can see.
    pub(crate) fn swap(&mut self, device: &Device, handle: Handle<Texture>, texture: Texture) {
        let bind_group = texture.create_bind_group(device, &self.layout);
        let raw_bind_group = texture.create_raw_bind_group(device, &self.layout);
        let slot = self
            .store
            .get_mut(handle)
            .expect("Textures never removes, so every handle it issued resolves");
        *slot = texture;
        self.bind_groups.insert(handle, bind_group);
        self.raw_bind_groups.insert(handle, raw_bind_group);
        // Real pixels now, whatever was there before.
        self.broken.remove(&handle);
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
        if is_render_texture(path.as_str()) {
            return self.reload_render_texture(device, handle, path);
        }

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

        self.swap(device, handle, texture);

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

    /// The bind group of `handle`'s texture seen through its linear view:
    /// for a texture used as data (a normal map), whose bytes must arrive as
    /// written. Every handle that has a [`bind_group`](Self::bind_group) has
    /// one of these too.
    pub fn raw_bind_group(&self, handle: Handle<Texture>) -> &BindGroup {
        self.raw_bind_groups
            .get(&handle)
            .expect("Textures inserts a raw bind group with every texture")
    }

    /// The texture `handle` names.
    ///
    /// Every handle this type hands out is valid: the placeholder is inserted
    /// at construction and nothing here ever removes from the store. Only a
    /// handle forged from a different store can reach the `expect`.
    pub fn get(&self, handle: Handle<Texture>) -> &Texture {
        self.try_get(handle)
            .expect("Textures never removes, so every handle it issued resolves")
    }

    /// The texture `handle` names, or `None` for a handle this store never
    /// issued.
    ///
    /// [`get`](Self::get)'s answer without the invariant, for a caller that
    /// holds a handle it did not obtain from this store — two stores in one
    /// process is a question, not a crash.
    pub fn try_get(&self, handle: Handle<Texture>) -> Option<&Texture> {
        self.store.get(handle)
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

    /// Whether no texture is stored.
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    /// Reads, decodes and uploads one file.
    fn read(
        &self,
        device: &Device,
        queue: &Queue,
        path: &AssetPath,
    ) -> Result<Texture, AssetError> {
        let image = decode_file(&self.root, path)?;
        self.upload(device, queue, path, &image)
    }

    /// Uploads pixels [`decode_file`] produced.
    pub(crate) fn upload(
        &self,
        device: &Device,
        queue: &Queue,
        path: &AssetPath,
        image: &DecodedImage,
    ) -> Result<Texture, AssetError> {
        Texture::from_decoded(device, queue, path.as_str(), image, self.filter).map_err(|source| {
            AssetError::Decode {
                path: self.root.join(path.as_str()),
                source,
            }
        })
    }

    /// Whether `path` has been asked for already.
    pub(crate) fn is_loaded(&self, path: &AssetPath) -> bool {
        self.by_path.contains_key(path)
    }

    pub(crate) fn root_path(&self) -> &Path {
        &self.root
    }
}

/// Reads and decodes `path` under `root`: the only place textures touch the
/// disk, and needing no device, safe on any thread.
pub(crate) fn decode_file(root: &Path, path: &AssetPath) -> Result<DecodedImage, AssetError> {
    // Safe to join because `AssetPath` has already refused anything that
    // could climb out of the root. That check lives in the constructor
    // precisely so it cannot be forgotten here.
    let full = root.join(path.as_str());
    let bytes = crate::fs::read(&full).map_err(|source| AssetError::Read {
        path: full.clone(),
        source,
    })?;
    decode_png(&bytes).map_err(|source| AssetError::Decode { path: full, source })
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
