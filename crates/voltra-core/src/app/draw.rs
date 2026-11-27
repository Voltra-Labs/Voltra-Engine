//! Turns the world into the renderer's draw list, and drives a frame to the
//! screen — straight to the window with no UI, or into an offscreen target
//! with the UI composited over the top.

use voltra_render::egui_backend::ScreenDescriptor;
use voltra_render::{Filter, Layer, Renderer};
use voltra_scene::sprite::sheets::Sheets;

use super::{App, UiFrame};

mod direct;
mod parts;
mod resolve;
pub(super) mod shots;
pub(super) mod world;

use parts::{clock_seconds, lighting, mesh_draws, prepare_text, ui_batch, ui_scale};

impl App {
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
            prefabs: super::spawning::prefab_store(
                &mut self.prefabs,
                &self.asset_root,
                &self.registry,
            ),
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
pub(super) fn record_gpu_timings(renderer: &mut Renderer) {
    for (pass, ms) in renderer.gpu_timings() {
        voltra_profile::gpu(pass, ms);
    }
}
