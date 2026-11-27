//! A load's textures: decodes asked for on the pool, and uploaded as they
//! arrive.

use voltra_assets::AssetPath;
use voltra_jobs::JobPool;

use super::super::decodes::Decodes;
use crate::app::App;

impl App {
    /// The textures of `paths` this load must wait for, each decode started
    /// unless another load already has. None without a texture store (a
    /// run with no window has nothing to upload to).
    pub(super) fn request_decodes(
        &self,
        paths: Vec<AssetPath>,
        decodes: &mut Decodes,
        jobs: &JobPool,
    ) -> Vec<AssetPath> {
        let Some(textures) = self.textures.as_ref() else {
            return Vec::new();
        };
        let wanted: Vec<AssetPath> = paths
            .into_iter()
            .filter(|path| decodes.is_pending(path) || textures.needs_decode(path))
            .collect();
        for path in &wanted {
            decodes.request(jobs, textures.root(), path);
        }
        wanted
    }

    /// Uploads pixels decoded on the pool, if there is still a device.
    pub(super) fn admit_decoded(&mut self, decoded: voltra_assets::DecodedTexture) {
        let (Some(textures), Some(renderer)) = (self.textures.as_mut(), self.renderer.as_ref())
        else {
            return;
        };
        let context = renderer.context();
        textures.admit_decoded(context.device(), context.queue(), decoded);
    }
}
