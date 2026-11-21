//! Textures made at runtime rather than read from a file — a glyph atlas —
//! and replacing their pixels.

use voltra_render::wgpu::{Device, Queue};
use voltra_render::{Filter, Texture};

use super::Textures;
use crate::handle::Handle;

impl Textures {
    /// Stores pixels made at runtime rather than read from a file — a glyph
    /// atlas — under a handle of their own, with no path.
    ///
    /// `reload` never touches it, and [`loaded`](Self::loaded) does not list
    /// it: nothing on disk stands behind it. Pixels that do not match the
    /// size are logged and stored as the placeholder checker, so the handle
    /// still resolves.
    pub fn insert_generated(
        &mut self,
        device: &Device,
        queue: &Queue,
        label: &str,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> Handle<Texture> {
        match Texture::from_rgba8(device, queue, label, rgba, width, height, Filter::Linear) {
            Ok(texture) => {
                let bind_group = texture.create_bind_group(device, &self.layout);
                let handle = self.store.insert(texture);
                self.bind_groups.insert(handle, bind_group);
                handle
            }
            Err(e) => {
                log::warn!("{label}: {e}; drawing the missing-texture checker instead");
                self.insert_broken(device, queue)
            }
        }
    }

    /// Replaces the pixels under `handle` — a generated texture that grew or
    /// changed — keeping the handle. Returns whether anything changed.
    ///
    /// A new texture and bind group rather than a write into the old one, so
    /// a size change is the same call as a content change. Pixels that do not
    /// match the size are logged and the previous texture stays.
    #[allow(clippy::too_many_arguments)]
    pub fn replace(
        &mut self,
        device: &Device,
        queue: &Queue,
        handle: Handle<Texture>,
        label: &str,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> bool {
        if self.store.get(handle).is_none() {
            log::warn!("{label}: replacing a texture handle this store never issued");
            return false;
        }
        let texture =
            match Texture::from_rgba8(device, queue, label, rgba, width, height, Filter::Linear) {
                Ok(texture) => texture,
                Err(e) => {
                    log::warn!("{label}: {e}; keeping the previous texture");
                    return false;
                }
            };
        let bind_group = texture.create_bind_group(device, &self.layout);
        if let Some(slot) = self.store.get_mut(handle) {
            *slot = texture;
        }
        self.bind_groups.insert(handle, bind_group);
        true
    }
}
