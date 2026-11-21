//! Frame orchestration: what gets recorded and submitted each tick.

use wgpu::SurfaceTarget;

use crate::camera::{screen_projection, Camera2D, CameraBinding};
use crate::context::GpuContext;
use crate::lines::ViewportBinding;
use crate::mesh::Mesh;
use crate::pass::MeshDraw;
use crate::post::{PostPass, PostSettings};
use crate::target::RenderTarget;
use crate::texture::Texture;
use crate::{pass, pipeline, texture};

/// Drives one frame at a time on top of a [`GpuContext`].
pub struct Renderer {
    ctx: GpuContext,
    flat_color: wgpu::RenderPipeline,
    camera_binding: CameraBinding,
    /// The screen-space UI's projection, in its own buffer so it and the
    /// world camera can both be written before one submit.
    ui_camera_binding: CameraBinding,
    /// Layout group 1 was built against. Exposed so a texture cache outside
    /// this crate can build bind groups that actually match the pipeline —
    /// a second layout with the same descriptor does not.
    texture_layout: wgpu::BindGroupLayout,
    /// Bound whenever geometry has no texture of its own. White multiplies to
    /// a no-op, so this is what lets one pipeline serve both cases.
    white_bind_group: wgpu::BindGroup,
    /// Draws overlay segments over an already-rendered target.
    lines: wgpu::RenderPipeline,
    /// Grades the drawn scene into the frame.
    post: PostPass,
    /// Where the world is drawn before the post pass on the window path,
    /// sized to the surface; made on the first frame.
    frame_target: Option<RenderTarget>,
    /// The size the line shader widens against, rewritten every frame.
    viewport_binding: ViewportBinding,
    pub camera: Camera2D,
    pub clear_color: wgpu::Color,
}

impl Renderer {
    pub fn new(target: impl Into<SurfaceTarget<'static>>, width: u32, height: u32) -> Self {
        let ctx = GpuContext::new(target, width, height);
        let camera_binding = CameraBinding::new(ctx.device());
        let ui_camera_binding = camera_binding.sharing_layout(ctx.device());
        let texture_layout = texture::bind_group_layout(ctx.device());
        let flat_color = pipeline::create_flat_color(
            ctx.device(),
            ctx.config().format,
            camera_binding.layout(),
            &texture_layout,
        );
        let white = Texture::white(ctx.device(), ctx.queue());
        let white_bind_group = white.create_bind_group(ctx.device(), &texture_layout);

        let viewport_binding = ViewportBinding::new(ctx.device());
        let lines = pipeline::create_lines(
            ctx.device(),
            ctx.config().format,
            camera_binding.layout(),
            viewport_binding.layout(),
        );

        let post = PostPass::new(ctx.device(), ctx.config().format);

        let camera = {
            let mut camera = Camera2D::default();
            camera.aspect = aspect_of(width, height);
            camera
        };

        Self {
            ctx,
            flat_color,
            camera_binding,
            ui_camera_binding,
            texture_layout,
            white_bind_group,
            lines,
            post,
            frame_target: None,
            viewport_binding,
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

    /// Draws `mesh` to the window through the post pass, then `ui` over it in
    /// the window's pixels — the HUD is not graded.
    ///
    /// The path a shipped game takes: no editor. The world goes into an
    /// offscreen target the size of the surface, and `post` grades it into
    /// the frame.
    pub fn render_mesh(
        &mut self,
        mesh: Option<&Mesh>,
        draws: &[MeshDraw<'_>],
        ui: Layer<'_>,
        post: &PostSettings,
    ) {
        let Some(frame) = self.ctx.acquire() else {
            return;
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let (width, height) = (self.ctx.config().width, self.ctx.config().height);
        let format = self.ctx.config().format;
        let device = self.ctx.device().clone();
        let target = self.frame_target.get_or_insert_with(|| {
            RenderTarget::new(
                &device,
                "frame-target",
                format,
                width,
                height,
                crate::texture::Filter::Linear,
            )
        });
        target.resize(&device, width, height);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame-encoder"),
        });
        let target = self.frame_target.as_ref().expect("made just above");
        self.record_scene(&mut encoder, target.view(), mesh, draws);
        self.post.record(
            &device,
            self.ctx.queue(),
            &mut encoder,
            target.view(),
            &view,
            post,
            aspect_of(width, height),
        );
        self.record_ui(&mut encoder, &view, width, height, ui);

        self.ctx.queue().submit(Some(encoder.finish()));
        self.ctx.present(frame);
    }

    /// Draws `mesh` into `target`, grades it into `output` with `post`, and
    /// draws `ui` over `output` ungraded.
    ///
    /// The editor's path: `output` becomes an image inside a viewport panel,
    /// which is why the scene needs its own size and aspect rather than the
    /// window's. The two targets must be the same size.
    pub fn render_scene(
        &mut self,
        target: &RenderTarget,
        output: &RenderTarget,
        mesh: Option<&Mesh>,
        draws: &[MeshDraw<'_>],
        ui: Layer<'_>,
        post: &PostSettings,
    ) {
        let mut encoder =
            self.ctx
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("scene-encoder"),
                });

        self.record_scene(&mut encoder, target.view(), mesh, draws);
        self.post.record(
            self.ctx.device(),
            self.ctx.queue(),
            &mut encoder,
            target.view(),
            output.view(),
            post,
            target.aspect(),
        );
        self.record_ui(
            &mut encoder,
            output.view(),
            output.width(),
            output.height(),
            ui,
        );
        self.ctx.queue().submit(Some(encoder.finish()));
    }

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

    /// The screen-space layer, over the scene, in the target's pixels.
    fn record_ui(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
        ui: Layer<'_>,
    ) {
        if ui.mesh.is_none() {
            return;
        }
        self.ui_camera_binding.upload_matrix(
            self.ctx.queue(),
            screen_projection(width as f32, height as f32),
        );
        pass::draw_mesh_batches_over(
            encoder,
            view,
            &self.flat_color,
            self.ui_camera_binding.bind_group(),
            ui.mesh,
            ui.draws,
        );
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

/// Geometry drawn in a target's own pixels over the scene - y down, origin
/// top-left - with the sprite pipeline. [`Layer::EMPTY`] draws nothing.
#[derive(Clone, Copy)]
pub struct Layer<'a> {
    pub mesh: Option<&'a Mesh>,
    pub draws: &'a [MeshDraw<'a>],
}

impl Layer<'_> {
    pub const EMPTY: Layer<'static> = Layer {
        mesh: None,
        draws: &[],
    };
}

/// Guards against the divide-by-zero a minimised window would otherwise cause.
fn aspect_of(width: u32, height: u32) -> f32 {
    width.max(1) as f32 / height.max(1) as f32
}
