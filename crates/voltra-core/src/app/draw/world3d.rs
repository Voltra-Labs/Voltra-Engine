//! A frame through a lone 3D camera: the world's meshes culled and grouped
//! into the depth pass, the HUD over them (stage 135).
//!
//! "Lone": an active [`Camera3D`] and no active 2D camera. A scene with
//! both draws its 2D camera as before and says once that it cannot draw
//! the two together yet — one frame of both worlds is stage 136.

use std::collections::HashSet;

use voltra_assets::Meshes;
use voltra_ecs::{Entity, World};
use voltra_jobs::JobPool;
use voltra_render::render3d::{MeshDraw3D, Scene3D};
use voltra_render::{wgpu, Layer, RenderTarget, Renderer};
use voltra_scene::camera::Shot;
use voltra_scene::camera3d::{self, Camera3D};
use voltra_scene::draw3d::PARALLEL_FROM;
use voltra_scene::{CameraClear, DrawList3D, MeshInstance3D};

/// What drawing the 3D world keeps from frame to frame.
#[derive(Debug, Default)]
pub(in crate::app) struct World3D {
    /// Every mesh by handle; made with the first 3D frame.
    meshes: Option<Meshes>,
    /// Last frame's draw list, rebuilt in place.
    list: DrawList3D,
    /// Threads to cull a long list across; made with the first long one.
    jobs: Option<JobPool>,
    /// Whether the scene was told it holds a 2D and a 3D camera.
    mixed_warned: bool,
    /// The cameras whose projection problem was said, and which: once per
    /// camera and problem this session.
    projection_warned: HashSet<(Entity, &'static str)>,
    /// Whether the editor's game view asked, during the last layout, to
    /// show the scene's 3D camera.
    pub game_view: bool,
}

impl World3D {
    /// The 3D camera the frame is drawn through alone, if there is one and
    /// `shots` — the frame's 2D cameras — is empty.
    pub fn lone_camera(&mut self, world: &World, shots: &[Shot]) -> Option<Entity> {
        let camera = camera3d::active(world)?;
        if !shots.is_empty() {
            if !self.mixed_warned {
                self.mixed_warned = true;
                log::warn!(
                    "the scene has an active Camera2D and an active Camera3D; a frame of both \
                     is not drawn yet, so the 2D camera draws"
                );
            }
            return None;
        }
        Some(camera)
    }

    /// Draws `world` through `camera` into `output` — the window for
    /// `None` — and `ui` over it.
    pub fn draw(
        &mut self,
        world: &World,
        renderer: &mut Renderer,
        camera: Entity,
        output: Option<&RenderTarget>,
        ui: Layer<'_>,
    ) {
        let (width, height) = match output {
            Some(target) => (target.width(), target.height()),
            None => {
                let config = renderer.context().config();
                (config.width, config.height)
            }
        };
        let aspect = width.max(1) as f32 / height.max(1) as f32;
        let Some(view) = camera3d::view(world, camera, aspect) else {
            return;
        };
        if let Some(problem) = world
            .get::<Camera3D>(camera)
            .and_then(|c| c.projection.problem())
        {
            if self.projection_warned.insert((camera, problem)) {
                log::warn!(
                    "the Camera3D on {camera:?} cannot draw as set: {problem}; drawn corrected"
                );
            }
        }
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
        self.list.rebuild(world, &view, meshes, jobs);
        // One per mesh and winding, not per instance: made each frame, as
        // it borrows this frame's meshes.
        let draws: Vec<MeshDraw3D<'_>> = self
            .list
            .draws
            .iter()
            .filter_map(|draw| {
                let mesh = meshes.gpu(draw.mesh)?;
                Some(MeshDraw3D {
                    mesh,
                    instances: draw.instances.clone(),
                    mirrored: draw.mirrored,
                })
            })
            .collect();
        let clear = match world.get::<Camera3D>(camera).map(|c| c.clear) {
            Some(CameraClear::Color([r, g, b, a])) => wgpu::Color {
                r: r.into(),
                g: g.into(),
                b: b.into(),
                a: a.into(),
            },
            _ => renderer.clear_color,
        };
        let scene = Scene3D {
            view,
            instances: &self.list.instances,
            draws: &draws,
            clear,
        };
        match output {
            Some(target) => renderer.render_3d_into(target, &scene, ui),
            None => renderer.render_3d(&scene, ui),
        }
    }
}
