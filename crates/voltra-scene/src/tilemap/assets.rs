//! A layer's asset paths turned into handles: its sheet, its normal map and
//! its slicing.

use voltra_assets::{AssetPath, Atlases, Textures};

use super::Tilemap;

impl Tilemap {
    /// Sets or clears the sheet path and refreshes its handle, as
    /// [`Sprite::set_texture`](crate::Sprite::set_texture) does.
    pub fn set_texture(
        &mut self,
        path: Option<AssetPath>,
        textures: &mut Textures,
        device: &voltra_render::wgpu::Device,
        queue: &voltra_render::wgpu::Queue,
    ) {
        self.texture_handle = path.as_ref().map(|path| textures.load(device, queue, path));
        self.texture = path;
    }

    /// Sets or clears the normal map's path and refreshes its handle, as
    /// [`Sprite::set_normal_map`](crate::Sprite::set_normal_map) does.
    pub fn set_normal_map(
        &mut self,
        path: Option<AssetPath>,
        textures: &mut Textures,
        device: &voltra_render::wgpu::Device,
        queue: &voltra_render::wgpu::Queue,
    ) {
        self.normal_handle = path.as_ref().map(|path| {
            let handle = textures.load(device, queue, path);
            if textures.is_broken(handle) {
                log::warn!(
                    "normal map {} did not load; the layer is shaded flat until it does",
                    path.as_str()
                );
            }
            handle
        });
        self.normal_map = path;
    }

    /// Sets or clears the atlas path and refreshes its handle, as
    /// [`Sprite::set_atlas`](crate::Sprite::set_atlas) does.
    pub fn set_atlas(&mut self, path: Option<AssetPath>, atlases: &mut Atlases) {
        self.atlas_handle = path.as_ref().map(|path| atlases.load(path));
        self.atlas = path;
    }
}
