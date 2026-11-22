//! Turns the world into the renderer's draw list, and drives a frame to the
//! screen — straight to the window with no UI, or into an offscreen target
//! with the UI composited over the top.

use voltra_assets::{Fonts, Textures};
use voltra_ecs::World;
use voltra_render::egui_backend::ScreenDescriptor;
use voltra_render::{wgpu, Filter, Layer, MeshDraw};
use voltra_scene::sprite::sheets::Sheets;
use voltra_scene::{ui, SpriteBatch, UiLayout};

use super::{App, UiFrame};

impl App {
    /// Draws the world straight to the window. No UI, no intermediate texture.
    pub(super) fn redraw_without_ui(&mut self) {
        self.step_gpu_particles();
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
        // Rebuilt every frame, but for tilemap chunks drawn last frame from
        // the same cells, which are copied: a level's tiles are the bulk of
        // its quads and change least.
        let sheets = Sheets::new(atlases, textures)
            .with_fonts(fonts)
            .seen_from(renderer.camera.position)
            .visible_in(view_min(&renderer.camera), view_max(&renderer.camera))
            .at_time(clock_seconds(self.elapsed))
            .with_layers(&self.sorting_layers);
        // Into last frame's buffers, taken out for the frame and put back.
        let mut batch = std::mem::take(&mut self.frame_batch);
        batch.rebuild(&self.world, sheets, &mut self.tile_meshes);
        let mesh = batch.upload(renderer.context().device());
        // Cloned (a cheap ref-count bump — `wgpu::BindGroup` is `Clone`)
        // rather than borrowed, so the borrow on `renderer` ends here instead
        // of surviving into the `&mut self` call below.
        let white = renderer.white_bind_group().clone();
        let draws = super::scene_draws::scene_draws(&batch, textures, &white, &self.gpu_rings);

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
        renderer.render_mesh(mesh.as_ref(), &draws, layer, &post);
        self.frame_batch = batch;
    }

    /// Scene to an offscreen target, then the UI over the top of the window.
    pub(super) fn redraw_with_ui(&mut self) {
        self.step_gpu_particles();
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
        prepare_text(&self.world, fonts, textures, &device, &queue, scale);
        // The immutable reborrow ends with the batch, well before the UI takes
        // both stores mutably below.
        let sheets = Sheets::new(atlases, textures)
            .with_fonts(fonts)
            .seen_from(renderer.camera.position)
            .visible_in(view_min(&renderer.camera), view_max(&renderer.camera))
            .at_time(clock_seconds(self.elapsed))
            .with_layers(&self.sorting_layers);
        // Into last frame's buffers, taken out for the frame and put back.
        let mut batch = std::mem::take(&mut self.frame_batch);
        batch.rebuild(&self.world, sheets, &mut self.tile_meshes);
        let mesh = batch.upload(&device);
        // Cloned (a cheap ref-count bump — `wgpu::BindGroup` is `Clone`)
        // rather than borrowed, so the borrow on `renderer` ends here instead
        // of surviving into the `&mut self` call below.
        let white = renderer.white_bind_group().clone();
        let draws = super::scene_draws::scene_draws(&batch, textures, &white, &self.gpu_rings);
        // The game's HUD over the scene, in the viewport's pixels: what the
        // window will show in a build, so the editor shows it too.
        let hud = ui_batch(&self.world, sheets, target.width(), target.height(), scale);
        let hud_mesh = hud.upload(&device);
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
        renderer.render_scene(target, graded, mesh.as_ref(), &draws, layer, &post);
        self.frame_batch = batch;

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

        let mut frame = UiFrame {
            world: &mut self.world,
            camera: &mut renderer.camera,
            textures,
            atlases,
            silhouettes: self.silhouettes.as_ref(),
            fonts,
            sorting_layers: &self.sorting_layers,
            clips,
            prefabs: super::spawning::prefab_store(&mut self.prefabs, &self.asset_root),
            animation_clips: super::animation::clip_store(
                &mut self.animation_clips,
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
        };
        egui.prepare(window, &device, &queue, screen, |root| ui(root, &mut frame));

        // After the UI, because the UI is what produced the segments, and
        // before `present_with`, because that is when egui samples the target.
        // The overlay therefore lands in the same frame as the scene under it.
        let lines = self.lines.upload(&device);
        renderer.render_lines(graded, lines.as_ref());

        renderer.present_with(|pass| egui.render(pass));
    }
}

/// Rasterizes the glyphs the world's texts draw and uploads the atlases
/// that changed — before the batch, so its UVs are taken against the atlas
/// as it will be sampled.
fn prepare_text(
    world: &World,
    fonts: &mut Fonts,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    ui_scale: f32,
) {
    voltra_scene::text::prepare(world, fonts, ui_scale);
    fonts.upload(textures, device, queue);
}

/// Physical pixels per point: the window's scale factor, or 1 before there
/// is a window to ask.
fn ui_scale(window: Option<&winit::window::Window>) -> f32 {
    window.map_or(1.0, |window| window.scale_factor() as f32)
}

/// The screen-space batch for a target `width` × `height` physical pixels.
///
/// Laid out in points — the size divided by `scale` — so a HUD authored at
/// 16 points in from the corner is 16 points in on any display.
fn ui_batch(world: &World, sheets: Sheets<'_>, width: u32, height: u32, scale: f32) -> SpriteBatch {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let screen = voltra_render::glam::Vec2::new(width as f32, height as f32) / scale;
    let layout = UiLayout::from_world(world, screen);
    ui::mesh::batch(world, sheets, &layout, scale)
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
            normal: None,
            light_layers: voltra_render::DEFAULT_LIGHT_LAYERS,
            indices: range.indices.clone(),
        })
        .collect()
}

/// The frame's lighting, with the lights `camera` can see lit. The editor
/// previews it too, as it previews the grade. Sprite-shaped casters' frames
/// are traced first, through `silhouettes`, when there are any.
fn lighting(
    world: &World,
    camera: &voltra_render::Camera2D,
    sheets: Sheets<'_>,
    silhouettes: Option<&mut voltra_assets::Silhouettes>,
    textures: &voltra_assets::Textures,
) -> voltra_render::Lighting {
    let silhouettes = silhouettes.map(|silhouettes| {
        voltra_scene::shadow_caster::prepare(world, sheets, silhouettes);
        &*silhouettes
    });
    let sprites = silhouettes.map(|silhouettes| voltra_scene::shadow_caster::SpriteShapes {
        sheets,
        silhouettes,
    });
    voltra_scene::light::gather(
        world,
        view_min(camera),
        view_max(camera),
        sprites,
        Some(textures),
    )
}

/// The lower-left corner of what `camera` sees, in world units.
fn view_min(camera: &voltra_render::Camera2D) -> voltra_render::glam::Vec2 {
    camera.position - camera.half_extents()
}

/// The upper-right corner of what `camera` sees, in world units.
fn view_max(camera: &voltra_render::Camera2D) -> voltra_render::glam::Vec2 {
    camera.position + camera.half_extents()
}

/// The elapsed clock as the batch takes it. Wrapped at a day, so an `f32`
/// keeps millisecond precision however long the game has been open.
fn clock_seconds(elapsed: f64) -> f32 {
    (elapsed % 86_400.0) as f32
}
