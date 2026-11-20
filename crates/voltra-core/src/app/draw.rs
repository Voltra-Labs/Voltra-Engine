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
        // Rebuilt every frame. Caching it means tracking every Transform and
        // Sprite write, which is not worth it until the batch actually shows up
        // in a profile.
        let sheets = Sheets::new(atlases, textures)
            .with_fonts(fonts)
            .seen_from(renderer.camera.position);
        let batch = SpriteBatch::from_world(&self.world, sheets);
        let mesh = batch.upload(renderer.context().device());
        // Cloned (a cheap ref-count bump — `wgpu::BindGroup` is `Clone`)
        // rather than borrowed, so the borrow on `renderer` ends here instead
        // of surviving into the `&mut self` call below.
        let white = renderer.white_bind_group().clone();
        let draws = mesh_draws(&batch, textures, &white);

        let config = renderer.context().config();
        let hud = ui_batch(&self.world, sheets, config.width, config.height, scale);
        let hud_mesh = hud.upload(renderer.context().device());
        let hud_draws = mesh_draws(&hud, textures, &white);
        let layer = Layer {
            mesh: hud_mesh.as_ref(),
            draws: &hud_draws,
        };
        renderer.render_mesh(mesh.as_ref(), &draws, layer);
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
            Some(atlases),
            Some(fonts),
            Some(clips),
        ) = (
            self.renderer.as_mut(),
            self.window.as_ref(),
            self.egui.as_mut(),
            self.scene_target.as_mut(),
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
        if target.resize(&device, width, height) {
            // The old view points at a texture that no longer exists, and the
            // scene has to be projected for the panel's shape, not the window's.
            egui.update_view(&device, viewport, target.raw_view(), Filter::Linear);
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
            .seen_from(renderer.camera.position);
        let batch = SpriteBatch::from_world(&self.world, sheets);
        let mesh = batch.upload(&device);
        // Cloned (a cheap ref-count bump — `wgpu::BindGroup` is `Clone`)
        // rather than borrowed, so the borrow on `renderer` ends here instead
        // of surviving into the `&mut self` call below.
        let white = renderer.white_bind_group().clone();
        let draws = mesh_draws(&batch, textures, &white);
        // The game's HUD over the scene, in the viewport's pixels: what the
        // window will show in a build, so the editor shows it too.
        let hud = ui_batch(&self.world, sheets, target.width(), target.height(), scale);
        let hud_mesh = hud.upload(&device);
        let hud_draws = mesh_draws(&hud, textures, &white);
        let layer = Layer {
            mesh: hud_mesh.as_ref(),
            draws: &hud_draws,
        };
        renderer.render_scene(target, mesh.as_ref(), &draws, layer);

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
            fonts,
            clips,
            prefabs: super::spawning::prefab_store(&mut self.prefabs, &self.asset_root),
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
        renderer.render_lines(target, lines.as_ref());

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
            indices: range.indices.clone(),
        })
        .collect()
}
