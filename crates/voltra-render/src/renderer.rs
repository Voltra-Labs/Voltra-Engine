//! Frame orchestration: what gets recorded and submitted each tick.

use wgpu::SurfaceTarget;

use crate::context::GpuContext;

/// Drives one frame at a time on top of a [`GpuContext`].
///
/// Today it only issues a clear pass. Draw submission lands here as the
/// pipeline layer grows.
pub struct Renderer {
    ctx: GpuContext,
    pub clear_color: wgpu::Color,
}

impl Renderer {
    pub fn new(target: impl Into<SurfaceTarget<'static>>, width: u32, height: u32) -> Self {
        Self {
            ctx: GpuContext::new(target, width, height),
            clear_color: wgpu::Color {
                r: 0.1,
                g: 0.1,
                b: 0.12,
                a: 1.0,
            },
        }
    }

    pub fn context(&self) -> &GpuContext {
        &self.ctx
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.ctx.resize(width, height);
    }

    pub fn render(&mut self) {
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
                    label: Some("frame-encoder"),
                });

        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear-pass"),
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
        }

        self.ctx.queue().submit(Some(encoder.finish()));
        self.ctx.present(frame);
    }
}
