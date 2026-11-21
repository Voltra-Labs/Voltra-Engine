//! Drawing the scene's normals into the renderer's normal target, on the
//! frames whose lighting reads them.

use super::Renderer;
use crate::mesh::Mesh;
use crate::normals::{NormalPass, NORMAL_FORMAT};
use crate::scene_draw::SceneDraw;
use crate::target::RenderTarget;
use crate::texture::Filter;

impl Renderer {
    /// Draws `draws`' normals into the normal target, `width` × `height`, if
    /// the frame's lighting shades normal maps and some range has one.
    /// Returns whether it did: when not, the light pass reads every light
    /// flat and the target is left as it was.
    pub(super) fn record_normals(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        (width, height): (u32, u32),
        mesh: Option<&Mesh>,
        draws: &[SceneDraw<'_>],
    ) -> bool {
        if !self.lighting.uses_normals() || !NormalPass::needed(draws) {
            return false;
        }
        let device = self.ctx.device();
        let target = self.normal_target.get_or_insert_with(|| {
            RenderTarget::new(
                device,
                "scene-normals",
                NORMAL_FORMAT,
                width,
                height,
                Filter::Nearest,
            )
        });
        target.resize(device, width, height);
        self.normals.record(
            encoder,
            target.view(),
            self.camera_binding.bind_group(),
            mesh,
            draws,
        );
        true
    }
}
