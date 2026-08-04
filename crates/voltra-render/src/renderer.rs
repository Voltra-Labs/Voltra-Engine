//! Frame orchestration: what gets recorded and submitted each tick.

use wgpu::SurfaceTarget;

use crate::camera::{Camera2D, CameraBinding};
use crate::context::GpuContext;
use crate::mesh::{self, Mesh};
use crate::{pass, pipeline};

/// Drives one frame at a time on top of a [`GpuContext`].
pub struct Renderer {
    ctx: GpuContext,
    flat_color: wgpu::RenderPipeline,
    camera_binding: CameraBinding,
    triangle: Mesh,
    pub camera: Camera2D,
    pub clear_color: wgpu::Color,
}

impl Renderer {
    pub fn new(target: impl Into<SurfaceTarget<'static>>, width: u32, height: u32) -> Self {
        let ctx = GpuContext::new(target, width, height);
        let camera_binding = CameraBinding::new(ctx.device());
        let flat_color =
            pipeline::create_flat_color(ctx.device(), ctx.config().format, camera_binding.layout());
        let triangle = Mesh::new(ctx.device(), "triangle", &mesh::TRIANGLE);

        Self {
            ctx,
            flat_color,
            camera_binding,
            triangle,
            camera: Camera2D {
                aspect: aspect_of(width, height),
                ..Default::default()
            },
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
        // Without this the image stretches when the window is not square.
        self.camera.aspect = aspect_of(width, height);
    }

    pub fn render(&mut self) {
        let Some(frame) = self.ctx.acquire() else {
            return;
        };

        // Uploaded every frame rather than on change: one 64-byte write is
        // cheaper than tracking whether the camera moved.
        self.camera_binding.upload(self.ctx.queue(), &self.camera);

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            self.ctx
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("frame-encoder"),
                });

        pass::draw_mesh(
            &mut encoder,
            &view,
            &self.flat_color,
            self.camera_binding.bind_group(),
            &self.triangle,
            self.clear_color,
        );

        self.ctx.queue().submit(Some(encoder.finish()));
        self.ctx.present(frame);
    }
}

/// Guards against the divide-by-zero a minimised window would otherwise cause.
fn aspect_of(width: u32, height: u32) -> f32 {
    width.max(1) as f32 / height.max(1) as f32
}
