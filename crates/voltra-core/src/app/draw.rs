//! Turns the world into the renderer's draw list, and drives a frame to the
//! screen — straight to the window with no UI, or into an offscreen target
//! with the UI composited over the top.

use voltra_render::egui_backend::ScreenDescriptor;
use voltra_render::{Filter, Layer, Renderer};
use voltra_scene::sprite::sheets::Sheets;

use super::{App, UiFrame};

mod parts;
pub(super) mod shots;
pub(super) mod world;

use parts::{
    clock_seconds, lighting, mesh_draws, prepare_text, ui_batch, ui_scale, view_max, view_min,
};

impl App {
    /// Draws the world straight to the window. No UI, no intermediate texture.
    pub(super) fn redraw_without_ui(&mut self) {
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
            .seen_from(renderer.camera.position)
            .visible_in(view_min(&renderer.camera), view_max(&renderer.camera))
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
        let bank = super::scene_draws::prepare_materials(
            &mut self.material_bank,
            renderer,
            format,
            &batch,
            self.materials.as_ref(),
        );
        let draws = super::scene_draws::scene_draws(
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

    /// Scene to an offscreen target, then the UI over the top of the window.
    pub(super) fn redraw_with_ui(&mut self) {
        let _draw = voltra_profile::scope("draw");
        self.step_gpu_particles();
        self.update_skinned_meshes();
        self.resolve_materials();
        self.resolve_text_icons();
        let (
            Some(renderer),
            Some(window),
            Some(egui),
            Some(target),
            Some(graded),
            Some(viewport),
            Some(ui),
            Some(textures),
            Some(atlases),
            Some(fonts),
            Some(clips),
        ) = (
            self.renderer.as_mut(),
            self.window.as_ref(),
            self.egui.as_mut(),
            self.scene_target.as_mut(),
            self.post_target.as_mut(),
            self.viewport,
            self.ui.as_mut(),
            self.textures.as_mut(),
            self.atlases.as_mut(),
            self.fonts.as_mut(),
            self.clips.as_mut(),
        )
        else {
            return;
        };
        // Every pass this frame records is timed while the profiler records.
        renderer.set_gpu_timing(voltra_profile::is_enabled());

        // Cloned handles rather than borrows: `renderer.camera` goes into the
        // UI callback mutably, which rules out holding a borrow of the renderer
        // across it. Both are reference-counted, so this costs a bump.
        let device = renderer.context().device().clone();
        let queue = renderer.context().queue().clone();

        let (width, height) = self.requested_size;
        let resized = target.resize(&device, width, height);
        if graded.resize(&device, width, height) || resized {
            // The old view points at a texture that no longer exists, and the
            // scene has to be projected for the panel's shape, not the window's.
            egui.update_view(&device, viewport, graded.raw_view(), Filter::Linear);
            renderer.camera.aspect = target.aspect();
        }

        // Before the layout that reads it, so a texture loaded by last frame's
        // layout is drawable by this one.
        self.thumbnails.sync(egui, &device, textures);

        let scale = window.scale_factor() as f32;
        // The scene's render textures, so a minimap shows in the editor too.
        // The editor's own view draws below, through the editor's camera.
        voltra_scene::camera::target::resolve_world(&mut self.world, textures, &device, &queue);
        let texture_shots: Vec<_> = voltra_scene::camera::shots(&self.world)
            .into_iter()
            .filter(|shot| shot.target.is_some())
            .collect();
        if !texture_shots.is_empty() {
            let editor_camera = renderer.camera;
            shots::ShotDraw {
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
            .draw(&texture_shots, (width.max(1), height.max(1)));
            renderer.camera = editor_camera;
        }
        world::WorldDraw {
            world: &self.world,
            renderer,
            textures,
            atlases,
            fonts,
            silhouettes: self.silhouettes.as_mut(),
            sorting_layers: &self.sorting_layers,
            batch: &mut self.frame_batch,
            buffers: &mut self.batch_buffers,
            tile_meshes: &mut self.tile_meshes,
            gpu_rings: &self.gpu_rings,
            skins: &self.skinned_meshes,
            materials: self.materials.as_ref(),
            material_bank: &mut self.material_bank,
            elapsed: self.elapsed,
            scale,
        }
        .into_targets(target, graded);

        let size = window.inner_size();
        let screen = ScreenDescriptor {
            width: size.width.max(1),
            height: size.height.max(1),
            pixels_per_point: window.scale_factor() as f32,
        };

        // Emptied before the layout that refills it, not after the draw: a
        // frame that returns early above must not leave last frame's segments
        // to be drawn again.
        self.lines.clear();

        let gpu_particles = renderer.particle_pipelines().is_some();
        let mut frame = UiFrame {
            world: &mut self.world,
            camera: &mut renderer.camera,
            textures,
            atlases,
            silhouettes: self.silhouettes.as_mut(),
            fonts,
            materials: self.materials.as_mut(),
            sorting_layers: &self.sorting_layers,
            clips,
            prefabs: super::spawning::prefab_store(&mut self.prefabs, &self.asset_root),
            animation_clips: super::animation::clip_store(
                &mut self.animation_clips,
                &self.asset_root,
            ),
            state_machines: super::animation::machine_store(
                &mut self.state_machines,
                &self.asset_root,
            ),
            audio: &mut self.audio,
            device: &device,
            queue: &queue,
            lines: &mut self.lines,
            contacts: self.physics_world.contacts(),
            simulation: &mut self.simulation,
            viewport,
            viewport_size: (target.width(), target.height()),
            requested_size: &mut self.requested_size,
            thumbnails: self.thumbnails.ids(),
            graded,
            actions: &self.actions,
            held_actions: &mut self.held_actions,
            window: &mut self.window_requests,
            registry: &mut self.registry,
            game: &mut self.game,
            net: &mut self.net,
            gpu_particles,
        };
        {
            let _ui = voltra_profile::scope("ui");
            egui.prepare(window, &device, &queue, screen, |root| ui(root, &mut frame));
        }
        if let Some(title) = self.window_requests.take_title() {
            window.set_title(&title);
        }

        // After the UI, because the UI is what produced the segments, and
        // before `present_with`, because that is when egui samples the target.
        // The overlay therefore lands in the same frame as the scene under it.
        let lines = self.lines.upload(&device);
        renderer.render_lines(graded, lines.as_ref());

        let _present = voltra_profile::scope("present");
        renderer.present_with(|pass| egui.render(pass));
        record_gpu_timings(renderer);
    }
}

/// Hands the profiler the GPU pass timings that have come back — measured a
/// frame or a few ago.
fn record_gpu_timings(renderer: &mut Renderer) {
    for (pass, ms) in renderer.gpu_timings() {
        voltra_profile::gpu(pass, ms);
    }
}

impl App {
    /// Points every sprite and tile layer naming a material at it — a
    /// spawned one, a path edited — before a draw reads them.
    pub(super) fn resolve_materials(&mut self) {
        if let Some(materials) = self.materials.as_mut() {
            voltra_scene::sprite_material::resolve_world(&mut self.world, materials);
        }
    }

    /// Loads the atlases and textures rich texts' `[icon=…]` tags name —
    /// every frame, as a language switch can bring an icon; a scan for the
    /// tag when nothing changed.
    pub(super) fn resolve_text_icons(&mut self) {
        if let (Some(renderer), Some(atlases), Some(textures)) = (
            self.renderer.as_ref(),
            self.atlases.as_mut(),
            self.textures.as_mut(),
        ) {
            voltra_scene::text::icons::resolve_world(
                &mut self.world,
                atlases,
                textures,
                renderer.context().device(),
                renderer.context().queue(),
            );
        }
    }
}
