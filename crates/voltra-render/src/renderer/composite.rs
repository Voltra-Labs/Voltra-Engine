//! Several cameras' pictures laid side by side on one output — split
//! screen, a picture in a picture — and the HUD over them.

use super::{Layer, Renderer};
use crate::camera::screen_projection;
use crate::mesh::{Mesh, Vertex};
use crate::pass::{self, MeshDraw};
use crate::target::RenderTarget;

/// One camera's finished picture, and the rectangle of the output it goes
/// in, in the output's pixels from its top-left.
pub struct Placed<'a> {
    /// The camera's finished picture.
    pub picture: &'a RenderTarget,
    /// Where it goes, as `[x, y, width, height]` in the output's pixels.
    pub rect: [f32; 4],
}

impl Renderer {
    /// Clears `output`, draws `pictures` over it in order — a later one
    /// over an earlier where they overlap — then `ui` over all of them.
    pub fn composite(&mut self, output: &RenderTarget, pictures: &[Placed<'_>], ui: Layer<'_>) {
        let device = self.ctx.device().clone();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("composite-encoder"),
        });
        // Held past the submit: a mesh's buffers are destroyed when it is
        // dropped, and the commands still name them.
        let _mesh = self.record_composite(
            &mut encoder,
            output.view(),
            (output.width(), output.height()),
            pictures,
            ui,
        );
        self.ctx.submit(encoder.finish());
    }

    /// [`composite`](Self::composite) into the window, and presents it.
    pub fn present_composite(&mut self, pictures: &[Placed<'_>], ui: Layer<'_>) {
        let Some(frame) = self.ctx.acquire() else {
            return;
        };
        let view = self.ctx.frame_view(&frame);
        let size = (self.ctx.config().width, self.ctx.config().height);
        let device = self.ctx.device().clone();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("present-composite-encoder"),
        });
        let _mesh = self.record_composite(&mut encoder, &view, size, pictures, ui);
        self.ctx.submit(encoder.finish());
        self.ctx.present(frame);
    }

    fn record_composite(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        (width, height): (u32, u32),
        pictures: &[Placed<'_>],
        ui: Layer<'_>,
    ) -> Option<Mesh> {
        let device = self.ctx.device();
        let mut vertices = Vec::with_capacity(pictures.len() * 4);
        let mut indices = Vec::with_capacity(pictures.len() * 6);
        for (i, placed) in pictures.iter().enumerate() {
            let [x, y, w, h] = placed.rect;
            let base = (i * 4) as u32;
            let corner = |px: f32, py: f32, u: f32, v: f32| Vertex {
                position: [px, py],
                color: [1.0; 4],
                uv: [u, v],
            };
            vertices.extend([
                corner(x, y, 0.0, 0.0),
                corner(x, y + h, 0.0, 1.0),
                corner(x + w, y + h, 1.0, 1.0),
                corner(x + w, y, 1.0, 0.0),
            ]);
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        let mesh =
            (!vertices.is_empty()).then(|| Mesh::indexed(device, "composite", &vertices, &indices));
        // The sRGB view: sampled as linear and written back encoded, as the
        // sprite pipeline writes everything else.
        let groups: Vec<wgpu::BindGroup> = pictures
            .iter()
            .map(|placed| {
                placed
                    .picture
                    .create_srgb_bind_group(device, &self.texture_layout)
            })
            .collect();
        let draws: Vec<MeshDraw<'_>> = groups
            .iter()
            .enumerate()
            .map(|(i, group)| MeshDraw {
                texture: group,
                normal: None,
                light_layers: crate::DEFAULT_LIGHT_LAYERS,
                primitives: crate::mesh::Primitives::Triangles((i as u32 * 6)..(i as u32 * 6 + 6)),
                material: None,
            })
            .collect();
        self.ui_camera_binding.upload_matrix(
            self.ctx.queue(),
            screen_projection(width as f32, height as f32),
        );
        pass::draw_mesh_batches(
            encoder,
            view,
            &self.flat_color,
            self.ui_camera_binding.bind_group(),
            mesh.as_ref(),
            &draws,
            self.clear_color,
        );
        if ui.mesh.is_some() {
            pass::draw_mesh_batches_over(
                encoder,
                view,
                &self.flat_color,
                self.ui_camera_binding.bind_group(),
                ui.mesh,
                ui.draws,
            );
        }
        mesh
    }
}
