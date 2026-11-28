//! A frame drawn straight to the window, with no UI over it.

use super::*;

impl App {
    /// Draws the world straight to the window. No UI, no intermediate texture.
    pub(in crate::app) fn redraw_without_ui(&mut self) {
        let _draw = voltra_profile::scope("draw");
        self.step_gpu_particles();
        self.update_skinned_meshes();
        self.resolve_materials();
        self.resolve_text_icons();
        let (Some(renderer), Some(textures), Some(atlases), Some(fonts)) = (
            self.renderer.as_mut(),
            self.textures.as_mut(),
            self.atlases.as_ref(),
            self.fonts.as_mut(),
        ) else {
            return;
        };
        // Before the batch, not after: the frame about to be recorded has to be
        // the one the scene's camera describes. `aspect` is the window's and
        // stays the window's — `Renderer::resize` owns it.
        self.framing.apply(&self.world, &mut renderer.camera);
        let scale = ui_scale(self.window.as_deref());
        prepare_text(
            &self.world,
            fonts,
            textures,
            renderer.context().device(),
            renderer.context().queue(),
            scale,
        );
        voltra_scene::camera::target::resolve_world(
            &mut self.world,
            textures,
            renderer.context().device(),
            renderer.context().queue(),
        );
        let frame_shots = voltra_scene::camera::shots(&self.world);
        if let Some(camera) = self.world3d.lone_camera(&self.world, &frame_shots) {
            let config = renderer.context().config();
            let screen = (config.width, config.height);
            let hud = shots::hud(&self.world, atlases, textures, fonts, screen, scale);
            let hud_mesh = hud.upload(renderer.context().device());
            let white = renderer.white_bind_group().clone();
            let hud_draws = mesh_draws(&hud, textures, &white);
            let _render = voltra_profile::scope("render");
            renderer.set_gpu_timing(voltra_profile::is_enabled());
            let hud = Layer {
                mesh: hud_mesh.as_ref(),
                draws: &hud_draws,
            };
            self.world3d.draw(&self.world, renderer, camera, None, hud);
            record_gpu_timings(renderer);
            return;
        }
        if shots::needs_shots(&frame_shots) {
            let config = renderer.context().config();
            let screen = (config.width, config.height);
            let rects = shots::ShotDraw {
                world: &self.world,
                renderer,
                textures,
                atlases,
                fonts,
                silhouettes: self.silhouettes.as_mut(),
                sorting_layers: &self.sorting_layers,
                batch: &mut self.frame_batch,
                tile_meshes: &mut self.tile_meshes,
                gpu_rings: &self.gpu_rings,
                skins: &self.skinned_meshes,
                materials: self.materials.as_ref(),
                material_bank: &mut self.material_bank,
                pool: &mut self.camera_pool,
                elapsed: self.elapsed,
            }
            .draw(&frame_shots, screen);
            let hud = shots::hud(&self.world, atlases, textures, fonts, screen, scale);
            let hud_mesh = hud.upload(renderer.context().device());
            let white = renderer.white_bind_group().clone();
            let hud_draws = mesh_draws(&hud, textures, &white);
            let placed = shots::placed(&self.camera_pool, &rects);
            let _render = voltra_profile::scope("render");
            renderer.present_composite(
                &placed,
                Layer {
                    mesh: hud_mesh.as_ref(),
                    draws: &hud_draws,
                },
            );
            return;
        }
        // Rebuilt every frame, but for tilemap chunks drawn last frame from
        // the same cells, which are copied: a level's tiles are the bulk of
        // its quads and change least.
        let sheets = Sheets::new(atlases, textures)
            .with_fonts(fonts)
            .through(&renderer.camera)
            .at_time(clock_seconds(self.elapsed))
            .with_layers(&self.sorting_layers);
        let sheets = match self.materials.as_ref() {
            Some(materials) => sheets.with_materials(materials),
            None => sheets,
        };
        // Into last frame's buffers, taken out for the frame and put back.
        let mut batch = std::mem::take(&mut self.frame_batch);
        batch.rebuild(&self.world, sheets, &mut self.tile_meshes);
        let mesh = batch.upload_into(
            renderer.context().device(),
            renderer.context().queue(),
            &mut self.batch_buffers,
        );
        // Cloned (a cheap ref-count bump — `wgpu::BindGroup` is `Clone`)
        // rather than borrowed, so the borrow on `renderer` ends here instead
        // of surviving into the `&mut self` call below.
        let white = renderer.white_bind_group().clone();
        let format = renderer.context().format();
        let bank = crate::app::scene_draws::prepare_materials(
            &mut self.material_bank,
            renderer,
            format,
            &batch,
            self.materials.as_ref(),
        );
        let draws = crate::app::scene_draws::scene_draws(
            &batch,
            textures,
            &white,
            (&self.gpu_rings, &self.skinned_meshes),
            bank,
            None,
        );

        let config = renderer.context().config();
        let hud = ui_batch(&self.world, sheets, config.width, config.height, scale);
        let hud_mesh = hud.upload(renderer.context().device());
        let hud_draws = mesh_draws(&hud, textures, &white);
        let layer = Layer {
            mesh: hud_mesh.as_ref(),
            draws: &hud_draws,
        };
        let post = voltra_scene::post::active(&self.world);
        renderer.lighting = lighting(
            &self.world,
            &renderer.camera,
            sheets,
            self.silhouettes.as_mut(),
            textures,
        );
        let _render = voltra_profile::scope("render");
        renderer.set_gpu_timing(voltra_profile::is_enabled());
        renderer.render_mesh(mesh, &draws, layer, &post);
        record_gpu_timings(renderer);
        self.frame_batch = batch;
    }
}
