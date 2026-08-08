//! Turns the world into the renderer's draw list, and drives a frame to the
//! screen — straight to the window with no UI, or into an offscreen target
//! with the UI composited over the top.

use voltra_assets::Textures;
use voltra_render::egui_backend::ScreenDescriptor;
use voltra_render::{wgpu, Filter, MeshDraw};
use voltra_scene::SpriteBatch;

use super::{App, UiFrame};

impl App {
    /// Draws the world straight to the window. No UI, no intermediate texture.
    pub(super) fn redraw_without_ui(&mut self) {
        let (Some(renderer), Some(textures)) = (self.renderer.as_mut(), self.textures.as_ref())
        else {
            return;
        };
        // Rebuilt every frame. Caching it means tracking every Transform and
        // Sprite write, which is not worth it until the batch actually shows up
        // in a profile.
        let batch = SpriteBatch::from_world(&self.world);
        let mesh = batch.upload(renderer.context().device());
        // Cloned (a cheap ref-count bump — `wgpu::BindGroup` is `Clone`)
        // rather than borrowed, so the borrow on `renderer` ends here instead
        // of surviving into the `&mut self` call below.
        let white = renderer.white_bind_group().clone();
        let draws = mesh_draws(&batch, textures, &white);
        renderer.render_mesh(mesh.as_ref(), &draws);
    }

    /// Scene to an offscreen target, then the UI over the top of the window.
    pub(super) fn redraw_with_ui(&mut self) {
        let (
            Some(renderer),
            Some(window),
            Some(egui),
            Some(target),
            Some(viewport),
            Some(ui),
            Some(textures),
        ) = (
            self.renderer.as_mut(),
            self.window.as_ref(),
            self.egui.as_mut(),
            self.scene_target.as_mut(),
            self.viewport,
            self.ui.as_mut(),
            self.textures.as_mut(),
        )
        else {
            return;
        };

        // Cloned handles rather than borrows: `renderer.camera` goes into the
        // UI callback mutably, which rules out holding a borrow of the renderer
        // across it. Both are reference-counted, so this costs a bump.
        let device = renderer.context().device().clone();
        let queue = renderer.context().queue().clone();

        let (width, height) = self.requested_size;
        if target.resize(&device, width, height) {
            // The old view points at a texture that no longer exists, and the
            // scene has to be projected for the panel's shape, not the window's.
            egui.update_view(&device, viewport, target.raw_view(), Filter::Linear);
            renderer.camera.aspect = target.aspect();
        }

        let batch = SpriteBatch::from_world(&self.world);
        let mesh = batch.upload(&device);
        // Cloned (a cheap ref-count bump — `wgpu::BindGroup` is `Clone`)
        // rather than borrowed, so the borrow on `renderer` ends here instead
        // of surviving into the `&mut self` call below.
        let white = renderer.white_bind_group().clone();
        let draws = mesh_draws(&batch, textures, &white);
        renderer.render_scene(target, mesh.as_ref(), &draws);

        let size = window.inner_size();
        let screen = ScreenDescriptor {
            width: size.width.max(1),
            height: size.height.max(1),
            pixels_per_point: window.scale_factor() as f32,
        };

        let mut frame = UiFrame {
            world: &mut self.world,
            camera: &mut renderer.camera,
            textures,
            device: &device,
            queue: &queue,
            viewport,
            viewport_size: (target.width(), target.height()),
            requested_size: &mut self.requested_size,
        };
        egui.prepare(window, &device, &queue, screen, |root| ui(root, &mut frame));

        renderer.present_with(|pass| egui.render(pass));
    }
}

/// Turns a batch's texture-run ranges into the renderer's draw list.
///
/// `white` stands in for `None`: an untextured range still needs a bind
/// group, and the renderer's own white one is what every flat-coloured
/// sprite drew against before textures existed. Both `textures` and `white`
/// share `'a` so the returned draws can borrow either.
fn mesh_draws<'a>(
    batch: &SpriteBatch,
    textures: &'a Textures,
    white: &'a wgpu::BindGroup,
) -> Vec<MeshDraw<'a>> {
    batch
        .ranges
        .iter()
        .map(|range| MeshDraw {
            texture: match range.texture {
                Some(handle) => textures.bind_group(handle),
                None => white,
            },
            indices: range.indices.clone(),
        })
        .collect()
}
