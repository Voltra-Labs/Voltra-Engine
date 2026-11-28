//! The frame paths: the world drawn, graded, and the HUD over it — into the
//! window or into an editor target.

use super::post_chain::{record_post, Chain};
use super::{Layer, Renderer};
use crate::camera::screen_projection;
use crate::mesh::Mesh2D;
use crate::pass;
use crate::post::PostSettings;
use crate::scene_draw::{self, SceneDraw};
use crate::target::RenderTarget;

impl Renderer {
    /// Draws `mesh` to the window through the post pass, then `ui` over it in
    /// the window's pixels — the HUD is not graded.
    ///
    /// The path a shipped game takes: no editor. The world goes into an
    /// offscreen target the size of the surface, and `post` grades it into
    /// the frame.
    pub fn render_mesh(
        &mut self,
        mesh: Option<&Mesh2D>,
        draws: &[SceneDraw<'_>],
        ui: Layer<'_>,
        post: &PostSettings,
    ) {
        let Some(frame) = self.ctx.acquire() else {
            return;
        };

        let view = self.ctx.frame_view(&frame);
        let (width, height) = (self.ctx.config().width, self.ctx.config().height);
        let format = self.ctx.format();
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
        let scope = self.gpu_timer.begin(&mut encoder, "scene");
        {
            let target = self.frame_target.as_ref().expect("made just above");
            self.record_scene(&mut encoder, target.view(), mesh, draws);
        }
        self.gpu_timer.end(&mut encoder, scope);
        let scope = self.gpu_timer.begin(&mut encoder, "normals");
        let normals = self.record_normals(&mut encoder, (width, height), mesh, draws);
        self.gpu_timer.end(&mut encoder, scope);
        let scope = self.gpu_timer.begin(&mut encoder, "light layers");
        let layers = self.record_layers(&mut encoder, (width, height), mesh, draws);
        self.gpu_timer.end(&mut encoder, scope);
        let target = self.frame_target.as_ref().expect("made just above");
        record_post(
            &self.ctx,
            Chain {
                lights: &self.lights,
                bloom: &self.bloom,
                grade: &self.post,
                camera: self.camera_binding.bind_group(),
                lighting: &self.lighting,
                // A field, not a method: the pool below is borrowed mutably.
                normals: self
                    .normal_target
                    .as_ref()
                    .filter(|_| normals)
                    .map(RenderTarget::view),
                layers: self
                    .layer_target
                    .as_ref()
                    .filter(|_| layers)
                    .map(RenderTarget::view),
            },
            &mut self.transients,
            &mut self.gpu_timer,
            &mut encoder,
            target.view(),
            &view,
            (width, height),
            post,
        );
        let scope = self.gpu_timer.begin(&mut encoder, "hud");
        self.record_ui(&mut encoder, &view, width, height, ui);
        self.gpu_timer.end(&mut encoder, scope);

        self.gpu_timer.finish(&mut encoder);
        self.ctx.submit(encoder.finish());
        self.gpu_timer.submitted();
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
        mesh: Option<&Mesh2D>,
        draws: &[SceneDraw<'_>],
        ui: Layer<'_>,
        post: &PostSettings,
    ) {
        let mut encoder =
            self.ctx
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("scene-encoder"),
                });

        let scope = self.gpu_timer.begin(&mut encoder, "scene");
        self.record_scene(&mut encoder, target.view(), mesh, draws);
        self.gpu_timer.end(&mut encoder, scope);
        let size = (target.width(), target.height());
        let scope = self.gpu_timer.begin(&mut encoder, "normals");
        let normals = self.record_normals(&mut encoder, size, mesh, draws);
        self.gpu_timer.end(&mut encoder, scope);
        let scope = self.gpu_timer.begin(&mut encoder, "light layers");
        let layers = self.record_layers(&mut encoder, size, mesh, draws);
        self.gpu_timer.end(&mut encoder, scope);
        record_post(
            &self.ctx,
            Chain {
                lights: &self.lights,
                bloom: &self.bloom,
                grade: &self.post,
                camera: self.camera_binding.bind_group(),
                lighting: &self.lighting,
                // A field, not a method: the pool below is borrowed mutably.
                normals: self
                    .normal_target
                    .as_ref()
                    .filter(|_| normals)
                    .map(RenderTarget::view),
                layers: self
                    .layer_target
                    .as_ref()
                    .filter(|_| layers)
                    .map(RenderTarget::view),
            },
            &mut self.transients,
            &mut self.gpu_timer,
            &mut encoder,
            target.view(),
            output.view(),
            (target.width(), target.height()),
            post,
        );
        let scope = self.gpu_timer.begin(&mut encoder, "hud");
        self.record_ui(
            &mut encoder,
            output.view(),
            output.width(),
            output.height(),
            ui,
        );
        self.gpu_timer.end(&mut encoder, scope);
        self.gpu_timer.finish(&mut encoder);
        self.ctx.submit(encoder.finish());
        self.gpu_timer.submitted();
    }

    /// The screen-space layer, over the scene, in the target's pixels.
    pub(super) fn record_ui(
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
        mesh: Option<&Mesh2D>,
        draws: &[SceneDraw<'_>],
    ) {
        // Uploaded every frame rather than on change: one 64-byte write is
        // cheaper than tracking whether the camera moved.
        self.camera_binding.upload(self.ctx.queue(), &self.camera);

        scene_draw::draw_scene(
            encoder,
            view,
            &self.flat_color,
            self.particles.as_ref(),
            self.camera_binding.bind_group(),
            mesh,
            draws,
            self.clear_color,
        );
    }
}
