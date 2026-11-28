//! A 3D camera's shot: the world's meshes culled and grouped into the
//! depth pass, each through its surface material and lit by the world's
//! light, through the camera's curve and grade, into the window or a
//! picture — the HUD over it when it is the frame's only screen shot.

use std::collections::HashSet;
use std::path::Path;

use voltra_assets::{Materials, Meshes, Textures};
use voltra_ecs::{Entity, World};
use voltra_jobs::JobPool;
use voltra_render::render3d::{MeshDraw3D, Scene3D};
use voltra_render::{wgpu, Layer, RenderTarget, Renderer};
use voltra_scene::camera::{FrameShots, Shot3D};
use voltra_scene::camera3d;
use voltra_scene::draw3d::PARALLEL_FROM;
use voltra_scene::{DrawList3D, MeshInstance3D};

use super::render_settings::{self, RenderSettings, DEFAULT_RENDER_SETTINGS};

mod clear;
mod lights;
mod previews;
mod surfaces;

use clear::clear;
pub(in crate::app) use clear::Picture3D;
pub(in crate::app) use previews::SurfacePreviews;
use surfaces::Surfaces;

/// What drawing the 3D world keeps from frame to frame.
#[derive(Debug, Default)]
pub(in crate::app) struct World3D {
    /// Every mesh by handle; made with the first 3D frame.
    meshes: Option<Meshes>,
    /// Last shot's draw list, rebuilt in place for each.
    list: DrawList3D,
    /// Threads to cull a long list across; made with the first long one.
    jobs: Option<JobPool>,
    /// `default.render.ron`, read with the first 3D frame.
    settings: Option<RenderSettings>,
    /// The cameras whose projection problem was said, and which: once per
    /// camera and problem this session.
    projection_warned: HashSet<(Entity, &'static str)>,
    /// The texture cameras said to draw with a texture's last frame.
    cycle_warned: HashSet<Entity>,
    /// The frame's surface materials on the GPU.
    surfaces: Surfaces,
    /// What has been said about the lights, once each: a light that cannot
    /// light, lights and environments past the one used.
    light_warned: HashSet<(Entity, &'static str)>,
    /// Materials drawn on spheres for the editor.
    pub previews: SurfacePreviews,
    /// Render textures named as a surface's image, said once each.
    image_warned: HashSet<voltra_assets::AssetPath>,
    /// Whether the editor's game view asked, during the last layout, to
    /// show the game's whole frame.
    pub game_view: bool,
}

impl World3D {
    /// Reads `default.render.ron` under `root` the first time a frame has a
    /// 3D camera, and hands the renderer its MSAA.
    pub fn prepare(&mut self, renderer: &mut Renderer, root: &Path) {
        let settings = *self
            .settings
            .get_or_insert_with(|| render_settings::read(root));
        renderer.set_msaa(settings.msaa);
    }

    /// Draws the material previews the layout asked for: see
    /// [`SurfacePreviews::draw`].
    pub fn draw_previews(
        &mut self,
        renderer: &mut Renderer,
        egui: &mut crate::ui::EguiLayer,
        textures: &mut Textures,
    ) {
        let meshes = self.meshes.get_or_insert_with(Meshes::new);
        let device = renderer.context().device().clone();
        let mut freed = Vec::new();
        self.previews.draw(
            renderer,
            textures,
            meshes,
            &mut |target: &RenderTarget| {
                egui.register_view(&device, target.raw_view(), voltra_render::Filter::Linear)
            },
            &mut |id| freed.push(id),
        );
        for id in freed {
            egui.free_view(id);
        }
    }

    /// Loads every mesh's surface material and the images it names: see
    /// [`voltra_scene::mesh_instance3d::resolve_world`].
    pub fn resolve_surfaces(
        &mut self,
        world: &World,
        materials: &mut Materials,
        textures: &mut Textures,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        if world.components::<MeshInstance3D>().is_none() {
            return;
        }
        voltra_scene::mesh_instance3d::resolve_world(
            world,
            materials,
            textures,
            device,
            queue,
            &mut self.image_warned,
        );
    }

    /// Reads the settings again if `path` is their file and they were read
    /// before; whether it was the file.
    pub fn reload_settings(&mut self, path: &voltra_assets::AssetPath, root: &Path) -> bool {
        if path.as_str() != DEFAULT_RENDER_SETTINGS {
            return false;
        }
        if self.settings.is_some() {
            self.settings = Some(render_settings::read(root));
            log::info!("reloaded {DEFAULT_RENDER_SETTINGS}");
        }
        true
    }

    /// Says, once per camera this session, which texture shots draw before
    /// a camera filling a texture their world may show: `frame`'s cycle.
    /// "May": the order is decided by what the world's sprites name, not by
    /// what each camera's view takes in.
    pub fn warn_cycles(&mut self, frame: &FrameShots) {
        for &camera in &frame.cycle {
            if self.cycle_warned.insert(camera) {
                log::warn!(
                    "the camera on {camera:?} draws into a render texture a sprite of its \
                     world may show, and so does another camera: it draws first, and if \
                     it sees that sprite, it sees the other texture's last frame"
                );
            }
        }
    }

    /// Draws `world` through `shot` into `output` — the window for `None`
    /// — and `ui` over it, as `picture` says: an overlay starts transparent
    /// where the camera's clear would have filled it, a render texture is
    /// written in straight alpha. `sources` are the world and the stores
    /// its surfaces draw from, resolved before the frame.
    pub fn draw(
        &mut self,
        renderer: &mut Renderer,
        sources: Sources3D<'_>,
        shot: &Shot3D,
        output: Option<&RenderTarget>,
        picture: Picture3D,
        ui: Layer<'_>,
    ) {
        let world = sources.world;
        let (width, height) = match output {
            Some(target) => (target.width(), target.height()),
            None => {
                let config = renderer.context().config();
                (config.width, config.height)
            }
        };
        let aspect = width.max(1) as f32 / height.max(1) as f32;
        let camera = shot.entity;
        let Some(view) = camera3d::view(world, camera, aspect) else {
            return;
        };
        if let Some(problem) = shot.camera.projection.problem() {
            if self.projection_warned.insert((camera, problem)) {
                log::warn!(
                    "the Camera3D on {camera:?} cannot draw as set: {problem}; drawn corrected"
                );
            }
        }
        let lighting = self.lighting(world, camera);
        let meshes = self.meshes.get_or_insert_with(Meshes::new);
        meshes.upload(renderer.context().device());
        let long = world
            .components::<MeshInstance3D>()
            .is_some_and(|all| all.len() >= PARALLEL_FROM);
        let jobs = if long {
            Some(
                &*self
                    .jobs
                    .get_or_insert_with(JobPool::with_available_parallelism),
            )
        } else {
            None
        };
        self.list
            .rebuild(world, &view, meshes, sources.materials, jobs);
        self.surfaces.prepare(
            renderer,
            sources.materials,
            sources.textures,
            &self.list.draws,
        );
        // One per mesh, material and winding, not per instance: made each
        // shot, as it borrows this frame's meshes and materials.
        let draws: Vec<MeshDraw3D<'_>> = self
            .list
            .draws
            .iter()
            .filter_map(|draw| {
                Some(MeshDraw3D {
                    mesh: meshes.gpu(draw.mesh)?,
                    material: self.surfaces.get(draw.material)?,
                    instances: draw.instances.clone(),
                    winding: draw.winding,
                })
            })
            .collect();
        let scene = Scene3D {
            lighting,
            view,
            instances: &self.list.instances,
            draws: &draws,
            clear: clear(
                shot.camera.clear,
                picture == Picture3D::Overlay,
                renderer.clear_color,
            ),
            time: sources.time,
        };
        let look = shot.camera.look();
        match (output, picture) {
            (Some(target), Picture3D::Texture) => renderer.render_3d_texture(target, &scene, &look),
            (Some(target), _) => renderer.render_3d_into(target, &scene, &look, ui),
            (None, _) => renderer.render_3d(&scene, &look, ui),
        }
    }
}

/// What a 3D shot draws from: the world, and the asset stores its
/// surfaces read — both resolved before the frame.
#[derive(Clone, Copy)]
pub(in crate::app) struct Sources3D<'a> {
    /// The world drawn.
    pub world: &'a World,
    /// The images surfaces sample.
    pub textures: &'a Textures,
    /// The surface materials; `None` before the asset root is known.
    pub materials: Option<&'a Materials>,
    /// What a surface's `in.time` reads: [`shader_seconds`] of the loop's
    /// clock.
    pub time: f32,
}

/// The loop's clock as a surface shader reads it: wrapped at an hour
/// ([`FrameUniform3D::TIME_WRAP`](voltra_render::render3d::FrameUniform3D::TIME_WRAP))
/// in `f64` first, so the `f32` keeps a quarter of a millisecond however
/// long the game has been open.
pub(in crate::app) fn shader_seconds(elapsed: f64) -> f32 {
    let wrap = f64::from(voltra_render::render3d::FrameUniform3D::TIME_WRAP);
    elapsed.rem_euclid(wrap) as f32
}
