//! A 3D camera's frame: the scene through the depth pass, then the HUD
//! over it — into the window or into a target.
//!
//! The direct path of a lone full-screen 3D camera (stage 135): no post
//! chain yet — HDR, MSAA, the tonemap and composing with 2D are the
//! one-frame-both-worlds stage's.

use super::{Layer, Renderer};
use crate::render3d::{Pass3D, Scene3D};
use crate::target::RenderTarget;

impl Renderer {
    /// Draws `scene` to the window, then `ui` over it in the window's
    /// pixels, and presents.
    pub fn render_3d(&mut self, scene: &Scene3D<'_>, ui: Layer<'_>) {
        let Some(frame) = self.ctx.acquire() else {
            return;
        };
        let view = self.ctx.frame_view(&frame);
        let (width, height) = (self.ctx.config().width, self.ctx.config().height);
        let format = self.ctx.format();
        self.record_3d(&view, format, (width, height), scene, ui);
        self.ctx.present(frame);
    }

    /// Draws `scene` into `output`, then `ui` over it in its pixels: the
    /// editor's and a headless screenshot's path.
    pub fn render_3d_into(&mut self, output: &RenderTarget, scene: &Scene3D<'_>, ui: Layer<'_>) {
        let size = (output.width(), output.height());
        self.record_3d(output.view(), output.format(), size, scene, ui);
    }

    fn record_3d(
        &mut self,
        view: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        (width, height): (u32, u32),
        scene: &Scene3D<'_>,
        ui: Layer<'_>,
    ) {
        let device = self.ctx.device().clone();
        let pass = match &mut self.pass3d {
            Some(pass) if pass.format() == format => pass,
            slot => slot.insert(Pass3D::new(&device, format)),
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame-3d-encoder"),
        });
        let scope = self.gpu_timer.begin(&mut encoder, "3d");
        pass.record(
            &device,
            self.ctx.queue(),
            &mut encoder,
            view,
            (width, height),
            scene,
        );
        self.gpu_timer.end(&mut encoder, scope);
        let scope = self.gpu_timer.begin(&mut encoder, "hud");
        self.record_ui(&mut encoder, view, width, height, ui);
        self.gpu_timer.end(&mut encoder, scope);
        self.gpu_timer.finish(&mut encoder);
        self.ctx.submit(encoder.finish());
        self.gpu_timer.submitted();
    }
}
