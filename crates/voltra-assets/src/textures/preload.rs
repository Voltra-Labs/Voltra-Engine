//! Many textures loaded at once: decoded across the job pool, uploaded in
//! order on the caller.

use std::collections::HashSet;

use voltra_jobs::JobPool;
use voltra_render::texture::DecodedImage;
use voltra_render::wgpu::{Device, Queue};

use super::{decode_file, Textures};
use crate::error::AssetError;
use crate::path::AssetPath;

impl Textures {
    /// Loads every path in `paths` not loaded yet, as [`Textures::load`]
    /// would one by one, and returns how many it loaded.
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
            .filter(|path| !self.is_loaded(path) && seen.insert(path.clone()))
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
}
