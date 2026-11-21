//! Passes drawn straight into a target, outside the frame's graph: the
//! editor's line overlay and the window's own layer.

use super::Renderer;
use crate::mesh::Mesh;
use crate::pass;
use crate::target::RenderTarget;

impl Renderer {
    /// Draws overlay segments over a target the scene is already in.
    ///
    /// A separate submission from [`Self::render_scene`], and deliberately
    /// after it: the pass loads rather than clears, so the scene has to be
    /// there first. It still lands before `present_with` samples the target,
    /// which is what keeps the overlay in the same frame as the scene it
    /// annotates rather than one behind it.
    ///
    /// The widths in `mesh` are in pixels, so the size uploaded here is the
    /// target's, not the window's.
    pub fn render_lines(&self, target: &RenderTarget, mesh: Option<&Mesh>) {
        if mesh.is_none() {
            return;
        }

        self.viewport_binding
            .upload(self.ctx.queue(), target.width(), target.height());

        let mut encoder =
            self.ctx
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("lines-encoder"),
                });

        pass::draw_lines(
            &mut encoder,
            target.view(),
            &self.lines,
            self.camera_binding.bind_group(),
            self.viewport_binding.bind_group(),
            mesh,
        );

        self.ctx.queue().submit(Some(encoder.finish()));
    }

    /// Acquires the window's frame, clears it, and hands the pass to `record`.
    ///
    /// Everything drawn over the top of the scene — the editor UI today —
    /// arrives through here rather than through a method per overlay.
    pub fn present_with(&mut self, record: impl FnOnce(&mut wgpu::RenderPass<'_>)) {
        let Some(frame) = self.ctx.acquire() else {
            return;
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            self.ctx
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("present-encoder"),
                });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            record(&mut pass);
        }

        self.ctx.queue().submit(Some(encoder.finish()));
        self.ctx.present(frame);
    }
}
