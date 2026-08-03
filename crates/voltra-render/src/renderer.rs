//! Frame orchestration: what gets recorded and submitted each tick.

use wgpu::SurfaceTarget;

use crate::context::GpuContext;
use crate::{pass, pipeline};

/// Drives one frame at a time on top of a [`GpuContext`].
pub struct Renderer {
    ctx: GpuContext,
    flat_color: wgpu::RenderPipeline,
    pub clear_color: wgpu::Color,
}

impl Renderer {
    pub fn new(target: impl Into<SurfaceTarget<'static>>, width: u32, height: u32) -> Self {
        let ctx = GpuContext::new(target, width, height);
        let flat_color = pipeline::create_flat_color(ctx.device(), ctx.config().format);

        Self {
            ctx,
            flat_color,
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

        pass::draw_flat_color(&mut encoder, &view, &self.flat_color, self.clear_color);

        self.ctx.queue().submit(Some(encoder.finish()));
        self.ctx.present(frame);
    }
}
