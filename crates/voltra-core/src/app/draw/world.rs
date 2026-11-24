//! The world drawn into an offscreen target, graded, with the HUD over it:
//! what the editor's viewport shows, and what a headless screenshot is.

use voltra_assets::{Atlases, Fonts, Silhouettes, Textures};
use voltra_ecs::World;
use voltra_render::{Layer, RenderTarget, Renderer};
use voltra_scene::sprite::sheets::Sheets;
use voltra_scene::{SortingLayers, SpriteBatch, TileMeshCache};

use super::parts::{
    clock_seconds, lighting, mesh_draws, prepare_text, ui_batch, view_max, view_min,
};
use crate::app::gpu_particles::GpuParticleRings;

/// Everything drawing the world needs, borrowed out of the app for one
/// frame. One copy of the draw, so the editor's viewport and a headless
/// screenshot cannot drift apart.
pub(in crate::app) struct WorldDraw<'a> {
    pub world: &'a World,
    pub renderer: &'a mut Renderer,
    pub textures: &'a mut Textures,
    pub atlases: &'a Atlases,
    pub fonts: &'a mut Fonts,
    pub silhouettes: Option<&'a mut Silhouettes>,
    pub sorting_layers: &'a SortingLayers,
    /// Last frame's batch, rebuilt in place.
    pub batch: &'a mut SpriteBatch,
    /// The GPU buffers it is written into, kept from frame to frame.
    pub buffers: &'a mut voltra_render::MeshBuffers,
    pub tile_meshes: &'a mut TileMeshCache,
    pub gpu_rings: &'a GpuParticleRings,
    /// The renderer's half of every skinned sprite.
    pub skins: &'a crate::app::skinned_meshes::SkinnedMeshes,
    /// The materials sprites name, and what they are drawn with.
    pub materials: Option<&'a voltra_assets::Materials>,
    pub material_bank: &'a mut Option<voltra_render::material::MaterialBank>,
    /// Seconds the app has run, for animated tiles.
    pub elapsed: f64,
    /// Physical pixels per point, for text and the HUD.
    pub scale: f32,
}

impl WorldDraw<'_> {
    /// Draws the world through the renderer's camera into `target`, grades
    /// it into `graded` and lays the HUD over that. The two targets are the
    /// same size.
    pub fn into_targets(self, target: &RenderTarget, graded: &RenderTarget) {
        let device = self.renderer.context().device().clone();
        let queue = self.renderer.context().queue().clone();
        prepare_text(
            self.world,
            self.fonts,
            self.textures,
            &device,
            &queue,
            self.scale,
        );
        let camera = self.renderer.camera;
        let sheets = Sheets::new(self.atlases, self.textures)
            .with_fonts(self.fonts)
            .seen_from(camera.position)
            .visible_in(view_min(&camera), view_max(&camera))
            .at_time(clock_seconds(self.elapsed))
            .with_layers(self.sorting_layers);
        let sheets = match self.materials {
            Some(materials) => sheets.with_materials(materials),
            None => sheets,
        };
        self.batch.rebuild(self.world, sheets, self.tile_meshes);
        let mesh = self.batch.upload_into(&device, &queue, self.buffers);
        let white = self.renderer.white_bind_group().clone();
        let bank = crate::app::scene_draws::prepare_materials(
            self.material_bank,
            self.renderer,
            target.format(),
            self.batch,
            self.materials,
        );
        let draws = crate::app::scene_draws::scene_draws(
            self.batch,
            self.textures,
            &white,
            (self.gpu_rings, self.skins),
            bank,
            None,
        );
        // The game's HUD over the scene, in the target's pixels: what the
        // window shows in a build.
        let hud = ui_batch(
            self.world,
            sheets,
            target.width(),
            target.height(),
            self.scale,
        );
        let hud_mesh = hud.upload(&device);
        let hud_draws = mesh_draws(&hud, self.textures, &white);
        let layer = Layer {
            mesh: hud_mesh.as_ref(),
            draws: &hud_draws,
        };
        let post = voltra_scene::post::active(self.world);
        self.renderer.lighting =
            lighting(self.world, &camera, sheets, self.silhouettes, self.textures);
        self.renderer
            .render_scene(target, graded, mesh, &draws, layer, &post);
    }
}
