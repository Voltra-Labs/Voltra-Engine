//! Many textures loaded at once: decoded across the job pool, uploaded in
//! order on the caller.

use std::collections::HashSet;
use std::path::Path;

use voltra_jobs::JobPool;
use voltra_render::texture::DecodedImage;
use voltra_render::wgpu::{Device, Queue};
use voltra_render::Texture;

use super::{decode_file, Textures};
use crate::error::AssetError;
use crate::handle::Handle;
use crate::path::AssetPath;

impl Textures {
    /// Loads every path in `paths` not loaded yet, as [`Textures::load`]
    /// would one by one, and returns how many it loaded. A render texture
    /// is made, not read: it is left to [`Textures::load`].
    ///
    /// Reading and decoding a PNG — the inflate — is most of a texture's
    /// load and needs no device, so those run across `jobs`; the uploads
    /// stay on the caller, in `paths`' order, so handles come out the same
    /// as loading them in a loop. A path that fails gets its private
    /// placeholder, exactly as `load` gives it. What a scene opens with is
    /// the case: dozens of sheets, each decoded on its own core.
    pub fn preload(
        &mut self,
        device: &Device,
        queue: &Queue,
        paths: impl IntoIterator<Item = AssetPath>,
        jobs: &JobPool,
    ) -> usize {
        let mut seen = HashSet::new();
        let mut decoded: Vec<(AssetPath, Option<Result<DecodedImage, AssetError>>)> = paths
            .into_iter()
            .filter(|path| self.needs_decode(path) && seen.insert(path.clone()))
            .map(|path| (path, None))
            .collect();
        let root = self.root_path().to_path_buf();
        jobs.for_each_chunk_mut(&mut decoded, 1, |_, chunk| {
            for (path, slot) in chunk {
                *slot = Some(decode_file(&root, path));
            }
        });

        let count = decoded.len();
        for (path, image) in decoded {
            let texture = image
                .expect("every slot is decoded before the scope returns")
                .and_then(|image| self.upload(device, queue, &path, &image));
            self.admit(device, queue, &path, texture);
        }
        count
    }

    /// Whether loading `path` would read and decode a file: it is not
    /// loaded yet and is not a render texture (which is made, not read).
    /// What a loader asks before handing the decode to a thread.
    pub fn needs_decode(&self, path: &AssetPath) -> bool {
        !self.is_loaded(path) && !super::is_render_texture(path.as_str())
    }

    /// Uploads what [`DecodedTexture::decode`] made elsewhere and caches it
    /// under its path, as [`Textures::load`] would have — a failed decode
    /// gets its private placeholder. A path loaded in the meantime keeps
    /// what it has and the pixels are dropped.
    pub fn admit_decoded(
        &mut self,
        device: &Device,
        queue: &Queue,
        decoded: DecodedTexture,
    ) -> Handle<Texture> {
        if let Some(handle) = self.by_path_handle(&decoded.path) {
            return handle;
        }
        let texture = decoded
            .image
            .and_then(|image| self.upload(device, queue, &decoded.path, &image));
        self.admit(device, queue, &decoded.path, texture)
    }
}

/// A texture's pixels, read and inflated with no device — on any thread —
/// waiting for [`Textures::admit_decoded`] to upload them. How a load that
/// spans frames keeps the decode off the frame.
#[derive(Debug)]
pub struct DecodedTexture {
    path: AssetPath,
    image: Result<DecodedImage, AssetError>,
}

impl DecodedTexture {
    /// Reads and decodes `path` under `root` — the store's
    /// [`root`](Textures::root) — or keeps why it would not.
    pub fn decode(root: &Path, path: AssetPath) -> Self {
        let image = decode_file(root, &path);
        Self { path, image }
    }

    /// The path it was read from.
    pub fn path(&self) -> &AssetPath {
        &self.path
    }
}
