//! Frame orchestration: what gets recorded and submitted each tick.

use wgpu::SurfaceTarget;

use crate::camera::{Camera2D, CameraBinding};
use crate::context::GpuContext;
use crate::mesh::Mesh;
use crate::pass::MeshDraw;
use crate::target::RenderTarget;
use crate::texture::Texture;
use crate::{pass, pipeline, texture};

/// Drives one frame at a time on top of a [`GpuContext`].
pub struct Renderer {
    ctx: GpuContext,
    flat_color: wgpu::RenderPipeline,
    camera_binding: CameraBinding,
    /// Layout group 1 was built against. Exposed so a texture cache outside
    /// this crate can build bind groups that actually match the pipeline —
    /// a second layout with the same descriptor does not.
    texture_layout: wgpu::BindGroupLayout,
    /// Bound whenever geometry has no texture of its own. White multiplies to
    /// a no-op, so this is what lets one pipeline serve both cases.
    white_bind_group: wgpu::BindGroup,
    pub camera: Camera2D,
    pub clear_color: wgpu::Color,
}

impl Renderer {
    pub fn new(target: impl Into<SurfaceTarget<'static>>, width: u32, height: u32) -> Self {
        let ctx = GpuContext::new(target, width, height);
        let camera_binding = CameraBinding::new(ctx.device());
        let texture_layout = texture::bind_group_layout(ctx.device());
        let flat_color = pipeline::create_flat_color(
            ctx.device(),
            ctx.config().format,
            camera_binding.layout(),
            &texture_layout,
        );
        let white = Texture::white(ctx.device(), ctx.queue());
        let white_bind_group = white.create_bind_group(ctx.device(), &texture_layout);

        let camera = {
            let mut camera = Camera2D::default();
            camera.aspect = aspect_of(width, height);
            camera
        };

        Self {
            ctx,
            flat_color,
            camera_binding,
            texture_layout,
            white_bind_group,
            camera,
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

    /// Layout every texture bind group must be built against.
    ///
    /// The pipeline was created with this exact layout object; a second one
    /// built from the same descriptor is a distinct type as far as wgpu's
    /// validation is concerned and will not bind against it.
    pub fn texture_layout(&self) -> &wgpu::BindGroupLayout {
        &self.texture_layout
    }

    /// The bind group drawn against when geometry has no texture of its own.
    pub fn white_bind_group(&self) -> &wgpu::BindGroup {
        &self.white_bind_group
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.ctx.resize(width, height);
        // Without this the image stretches when the window is not square.
        self.camera.aspect = aspect_of(width, height);
    }

    /// Draws `mesh` straight to the window, or just clears it.
    ///
    /// The path a shipped game takes: no editor, no intermediate texture.
    pub fn render_mesh(&mut self, mesh: Option<&Mesh>, draws: &[MeshDraw<'_>]) {
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

        self.record_scene(&mut encoder, &view, mesh, draws);

        self.ctx.queue().submit(Some(encoder.finish()));
        self.ctx.present(frame);
    }

    /// Draws `mesh` into an offscreen target instead of the window.
    ///
    /// The editor's path: the result becomes an image inside a viewport panel,
    /// which is why the scene needs its own size and aspect rather than the
    /// window's.
    pub fn render_scene(
        &mut self,
        target: &RenderTarget,
        mesh: Option<&Mesh>,
        draws: &[MeshDraw<'_>],
    ) {
        let mut encoder =
            self.ctx
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("scene-encoder"),
                });

        self.record_scene(&mut encoder, target.view(), mesh, draws);
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

    fn record_scene(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        mesh: Option<&Mesh>,
        draws: &[MeshDraw<'_>],
    ) {
        // Uploaded every frame rather than on change: one 64-byte write is
        // cheaper than tracking whether the camera moved.
        self.camera_binding.upload(self.ctx.queue(), &self.camera);

        pass::draw_mesh_batches(
            encoder,
            view,
            &self.flat_color,
            self.camera_binding.bind_group(),
            mesh,
            draws,
            self.clear_color,
        );
    }
}

/// Guards against the divide-by-zero a minimised window would otherwise cause.
fn aspect_of(width: u32, height: u32) -> f32 {
    width.max(1) as f32 / height.max(1) as f32
}
