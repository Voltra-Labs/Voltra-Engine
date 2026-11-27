//! A frame of several cameras: each drawn through its own view into its
//! own picture — a render texture, or a part of the screen — then the
//! screen's laid side by side with the HUD over them.

use voltra_assets::{Atlases, Fonts, Materials, Silhouettes, Textures};
use voltra_ecs::World;
use voltra_render::material::MaterialBank;
use voltra_render::{Layer, RenderTarget, Renderer};
use voltra_scene::camera::Shot;
use voltra_scene::sprite::sheets::Sheets;
use voltra_scene::{SortingLayers, SpriteBatch, TileMeshCache};

use super::parts::clock_seconds;
use crate::app::gpu_particles::GpuParticleRings;
use crate::app::scene_draws::{prepare_materials, scene_draws};

/// Everything drawing the shots needs, borrowed for one frame.
pub(in crate::app) struct ShotDraw<'a> {
    pub world: &'a World,
    pub renderer: &'a mut Renderer,
    pub textures: &'a Textures,
    pub atlases: &'a Atlases,
    pub fonts: &'a Fonts,
    pub silhouettes: Option<&'a mut Silhouettes>,
    pub sorting_layers: &'a SortingLayers,
    pub batch: &'a mut SpriteBatch,
    pub tile_meshes: &'a mut TileMeshCache,
    pub gpu_rings: &'a GpuParticleRings,
    /// The renderer's half of every skinned sprite.
    pub skins: &'a crate::app::skinned_meshes::SkinnedMeshes,
    pub materials: Option<&'a Materials>,
    pub material_bank: &'a mut Option<MaterialBank>,
    /// The scene and picture targets of the screen's cameras, by place in
    /// the frame's screen shots, kept from frame to frame.
    pub pool: &'a mut Vec<(RenderTarget, RenderTarget)>,
    pub elapsed: f64,
}

impl ShotDraw<'_> {
    /// Draws every one of `shots`: into its render texture, or into a
    /// picture the size of its viewport on a `screen` of that many pixels.
    /// Answers, for the screen's shots in order, which of `pool`'s pictures
    /// holds it and where on the screen it goes.
    pub fn draw(&mut self, shots: &[Shot], screen: (u32, u32)) -> Vec<(usize, [f32; 4])> {
        let device = self.renderer.context().device().clone();
        let format = self.renderer.context().format();
        let mut placed = Vec::new();
        for shot in shots {
            let (width, height, at) = match shot.target {
                Some(handle) => {
                    let Some(texture) = self.textures.render_texture(handle) else {
                        continue;
                    };
                    (texture.picture.width(), texture.picture.height(), None)
                }
                None => {
                    let [x, y, w, h] = shot.camera.viewport.pixels(screen.0, screen.1);
                    let slot = placed.len();
                    if self.pool.len() <= slot {
                        let make = |label| {
                            RenderTarget::new(
                                &device,
                                label,
                                format,
                                w,
                                h,
                                voltra_render::texture::Filter::Nearest,
                            )
                        };
                        self.pool
                            .push((make("camera-scene"), make("camera-picture")));
                    }
                    let (scene, picture) = &mut self.pool[slot];
                    scene.resize(&device, w, h);
                    picture.resize(&device, w, h);
                    placed.push((slot, [x as f32, y as f32, w as f32, h as f32]));
                    (w, h, Some(slot))
                }
            };
            self.draw_one(shot, width, height, at);
        }
        placed
    }

    /// One shot through its view, into its render texture or `pool[at]`.
    fn draw_one(&mut self, shot: &Shot, width: u32, height: u32, at: Option<usize>) {
        let view = shot.view(width, height);
        self.renderer.camera = view;
        let sheets = Sheets::new(self.atlases, self.textures)
            .with_fonts(self.fonts)
            .through(&view)
            .at_time(clock_seconds(self.elapsed))
            .with_layers(self.sorting_layers);
        let sheets = match self.materials {
            Some(materials) => sheets.with_materials(materials),
            None => sheets,
        };
        self.batch.rebuild(self.world, sheets, self.tile_meshes);
        let device = self.renderer.context().device().clone();
        let mesh = self.batch.upload(&device);
        let white = self.renderer.white_bind_group().clone();
        let format = self.renderer.context().format();
        let bank = prepare_materials(
            self.material_bank,
            self.renderer,
            format,
            self.batch,
            self.materials,
        );
        // A camera drawing into a texture does not see what shows it: a
        // texture is never sampled while it is drawn into.
        let draws = scene_draws(
            self.batch,
            self.textures,
            &white,
            (self.gpu_rings, self.skins),
            bank,
            shot.target,
        );
        let silhouettes = self.silhouettes.as_deref_mut().map(|silhouettes| {
            voltra_scene::shadow_caster::prepare(self.world, sheets, silhouettes);
            &*silhouettes
        });
        let sprites = silhouettes.map(|silhouettes| voltra_scene::shadow_caster::SpriteShapes {
            sheets,
            silhouettes,
        });
        self.renderer.lighting = voltra_scene::light::gather_through(
            self.world,
            &shot.camera,
            view.visible_bounds().0,
            view.visible_bounds().1,
            sprites,
            Some(self.textures),
        );
        let post = shot.camera.post.settings();
        let none = Layer {
            mesh: None,
            draws: &[],
        };
        let (scene, picture) = match (shot.target, at) {
            (Some(handle), _) => match self.textures.render_texture(handle) {
                Some(texture) => (&texture.scene, &texture.picture),
                None => return,
            },
            (None, Some(slot)) => {
                let (scene, picture) = &self.pool[slot];
                (scene, picture)
            }
            (None, None) => return,
        };
        self.renderer
            .render_scene(scene, picture, mesh.as_ref(), &draws, none, &post);
    }
}

/// Whether `shots` need the several-camera path: more than one, a render
/// texture, a part of the screen, or pixel-perfect snapping. One plain
/// full-screen camera keeps the direct path, as before.
pub(in crate::app) fn needs_shots(shots: &[Shot]) -> bool {
    match shots {
        [] => false,
        [one] => {
            one.target.is_some()
                || !one.camera.viewport.clamped().is_full()
                || one.camera.pixel_perfect
        }
        _ => true,
    }
}

/// The HUD over a `size` output at `scale` pixels per point.
pub(in crate::app) fn hud(
    world: &World,
    atlases: &Atlases,
    textures: &Textures,
    fonts: &Fonts,
    size: (u32, u32),
    scale: f32,
) -> SpriteBatch {
    let sheets = Sheets::new(atlases, textures).with_fonts(fonts);
    super::parts::ui_batch(world, sheets, size.0, size.1, scale)
}

/// The screen's pictures, from `pool`, where [`ShotDraw::draw`] placed them.
pub(in crate::app) fn placed<'p>(
    pool: &'p [(RenderTarget, RenderTarget)],
    rects: &[(usize, [f32; 4])],
) -> Vec<voltra_render::Placed<'p>> {
    rects
        .iter()
        .map(|&(slot, rect)| voltra_render::Placed {
            picture: &pool[slot].1,
            rect,
        })
        .collect()
}
