//! Render textures: a texture a camera draws into and a sprite shows.

use voltra_render::wgpu::{Device, Queue};
use voltra_render::{Filter, RenderTarget, Texture};

use super::Textures;
use crate::error::AssetError;
use crate::handle::Handle;
use crate::path::AssetPath;

/// The suffix that marks a RON file as a render texture:
/// `minimap.rendertexture.ron`.
pub const RENDER_TEXTURE_SUFFIX: &str = ".rendertexture.ron";

/// Whether a file name is a render texture.
pub fn is_render_texture(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(RENDER_TEXTURE_SUFFIX)
}

/// What a render texture file holds: its size in pixels, and how it is
/// filtered when shown larger or smaller.
///
/// ```ron
/// (version: 1, width: 256, height: 256, nearest: true)
/// ```
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RenderTextureFile {
    pub version: u32,
    pub width: u32,
    pub height: u32,
    /// Blocky when scaled: a minimap of pixel art.
    #[serde(default)]
    pub nearest: bool,
}

/// The two targets a camera draws a render texture through: the scene, and
/// the graded picture sprites sample.
pub struct RenderTexture {
    pub scene: RenderTarget,
    pub picture: RenderTarget,
}

/// The largest side accepted: a typo of a zero too many must not ask the
/// device for gigabytes.
const MAX_SIDE: u32 = 4096;

impl Textures {
    /// Loads the render texture at `path`: its targets made at the size the
    /// file says, and a handle sprites show its picture through. A file
    /// that will not read gives the checker, as an image would.
    pub(super) fn load_render_texture(
        &mut self,
        device: &Device,
        queue: &Queue,
        path: &AssetPath,
    ) -> Handle<Texture> {
        let made = self.read_render_texture(device, path);
        match made {
            Ok((targets, texture)) => {
                let handle = self.insert(device, texture);
                self.targets.insert(handle, targets);
                self.by_path.insert(path.clone(), handle);
                handle
            }
            Err(e) => self.admit(device, queue, path, Err(e)),
        }
    }

    /// Makes `handle`'s targets again from its file: a size edited.
    pub(super) fn reload_render_texture(
        &mut self,
        device: &Device,
        handle: Handle<Texture>,
        path: &AssetPath,
    ) -> bool {
        match self.read_render_texture(device, path) {
            Ok((targets, texture)) => {
                self.swap(device, handle, texture);
                self.targets.insert(handle, targets);
                self.broken.remove(&handle);
                log::info!("reloaded {}", path.as_str());
                true
            }
            Err(e) => {
                log::warn!("{e}; keeping the render texture as it was");
                false
            }
        }
    }

    fn read_render_texture(
        &self,
        device: &Device,
        path: &AssetPath,
    ) -> Result<(RenderTexture, Texture), AssetError> {
        let full = self.root.join(path.as_str());
        let text = crate::fs::read_to_string(&full).map_err(|source| AssetError::Read {
            path: full.clone(),
            source,
        })?;
        let file: RenderTextureFile = ron::from_str(&text).map_err(|source| AssetError::Parse {
            path: full.clone(),
            source: Box::new(source),
        })?;
        let (width, height) = (
            file.width.clamp(1, MAX_SIDE),
            file.height.clamp(1, MAX_SIDE),
        );
        let filter = if file.nearest {
            Filter::Nearest
        } else {
            Filter::Linear
        };
        let make = |label: &str| {
            RenderTarget::new(device, label, self.target_format, width, height, filter)
        };
        let targets = RenderTexture {
            scene: make("render-texture-scene"),
            picture: make(path.as_str()),
        };
        let texture = Texture::of_target(device, &targets.picture, filter);
        Ok((targets, texture))
    }

    /// The targets a camera draws `handle` through, if it is a render
    /// texture.
    pub fn render_texture(&self, handle: Handle<Texture>) -> Option<&RenderTexture> {
        self.targets.get(&handle)
    }
}
