//! A 3D camera's shot: the world's meshes culled and grouped into the
//! depth pass, through the camera's curve and grade, into the window or a
//! picture — the HUD over it when it is the frame's only screen shot.

use std::collections::HashSet;
use std::path::Path;

use voltra_assets::Meshes;
use voltra_ecs::{Entity, World};
use voltra_jobs::JobPool;
use voltra_render::render3d::{MeshDraw3D, Scene3D};
use voltra_render::{wgpu, Layer, RenderTarget, Renderer};
use voltra_scene::camera::{FrameShots, Shot3D};
use voltra_scene::camera3d;
use voltra_scene::draw3d::PARALLEL_FROM;
use voltra_scene::{CameraClear, DrawList3D, MeshInstance3D};

use super::render_settings::{self, RenderSettings, DEFAULT_RENDER_SETTINGS};

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
    /// — and `ui` over it. An `overlay` starts transparent where the
    /// camera's clear would have filled it.
    pub fn draw(
        &mut self,
        world: &World,
        renderer: &mut Renderer,
        shot: &Shot3D,
        output: Option<&RenderTarget>,
        overlay: bool,
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
        // One per mesh and winding, not per instance: made each shot, as
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
                    winding: draw.winding,
                })
            })
            .collect();
        let scene = Scene3D {
            view,
            instances: &self.list.instances,
            draws: &draws,
            clear: clear(shot.camera.clear, overlay, renderer.clear_color),
        };
        let look = shot.camera.look();
        match output {
            Some(target) => renderer.render_3d_into(target, &scene, &look, ui),
            None => renderer.render_3d(&scene, &look, ui),
        }
    }
}

/// What a 3D shot's target is cleared to, premultiplied as the picture
/// is: the camera's own colour; else transparent for an overlay, so what
/// drew before shows around its world; else the renderer's background.
fn clear(clear: CameraClear, overlay: bool, background: wgpu::Color) -> wgpu::Color {
    match clear {
        CameraClear::Color([r, g, b, a]) => {
            let a = a.clamp(0.0, 1.0);
            wgpu::Color {
                r: f64::from(r * a),
                g: f64::from(g * a),
                b: f64::from(b * a),
                a: f64::from(a),
            }
        }
        CameraClear::Background if overlay => wgpu::Color::TRANSPARENT,
        CameraClear::Background => background,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_overlay_clears_transparent_and_a_colour_of_its_own_stays() {
        let background = wgpu::Color {
            r: 0.1,
            g: 0.1,
            b: 0.12,
            a: 1.0,
        };
        assert_eq!(
            clear(CameraClear::Background, true, background),
            wgpu::Color::TRANSPARENT
        );
        assert_eq!(
            clear(CameraClear::Background, false, background),
            background
        );
        let half = clear(CameraClear::Color([1.0, 0.5, 0.0, 0.5]), true, background);
        assert_eq!((half.r, half.g, half.b, half.a), (0.5, 0.25, 0.0, 0.5));
    }
}
