//! Drawing the scene's light layers into the renderer's layer target, on
//! the frames whose lighting reads them.

use super::Renderer;
use crate::light_layers::{LayerPass, LAYER_FORMAT};
use crate::mesh::Mesh;
use crate::scene_draw::SceneDraw;
use crate::target::RenderTarget;
use crate::texture::Filter;

impl Renderer {
    /// Draws `draws`' light layers into the layer target, `width` × `height`,
    /// if some light of the frame leaves a layer out or some surface is on
    /// none. Returns whether it did:
    /// when not, the light pass puts every pixel on every layer.
    pub(super) fn record_layers(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        (width, height): (u32, u32),
        mesh: Option<&Mesh>,
        draws: &[SceneDraw<'_>],
    ) -> bool {
        let needed = self.lighting.uses_layers()
            || (self.lighting.ambient.is_some() && LayerPass::any_on_no_layer(draws));
        if !needed {
            return false;
        }
        let device = self.ctx.device();
        let target = self.layer_target.get_or_insert_with(|| {
            RenderTarget::new(
                device,
                "scene-light-layers",
                LAYER_FORMAT,
                width,
                height,
                Filter::Nearest,
            )
        });
        target.resize(device, width, height);
        self.layers.record(
            device,
            encoder,
            target.view(),
            self.camera_binding.bind_group(),
            mesh,
            draws,
        );
        true
    }
}
