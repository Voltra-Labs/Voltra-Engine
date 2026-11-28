//! Which path a frame takes, by its cameras, and the frame drawn when one
//! of them is 3D: a lone full-screen 3D camera straight into the output,
//! or every shot into a picture of its own and the pictures composited,
//! the HUD over them once.

use std::path::Path;

use voltra_render::{Layer, RenderTarget};
use voltra_scene::camera::{AnyShot, FrameShot, FrameShots, Shot, Shot3D};

use super::parts::mesh_draws;
use super::shots::{self, ShotDraw};
use super::world3d::{Picture3D, World3D};

/// How a frame is drawn, by [`plan`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app) enum Plan {
    /// No 3D camera: the 2D path, exactly as before 3D.
    Flat,
    /// One full-screen 3D camera is the screen's only shot (texture shots
    /// aside): drawn straight into the output, the HUD over it.
    Lone3D(Shot3D),
    /// Several shots on the screen, of both dimensions or a 3D part of it:
    /// each into its picture, composited.
    Composite,
}

/// The path `frame` takes.
pub(in crate::app) fn plan(frame: &FrameShots) -> Plan {
    if !frame.has_3d() {
        return Plan::Flat;
    }
    let mut screen = frame.screen();
    match (screen.next(), screen.next()) {
        (
            Some(FrameShot {
                shot: AnyShot::D3(shot),
                ..
            }),
            None,
        ) if shot.camera.viewport.clamped().is_full() => Plan::Lone3D(*shot),
        _ => Plan::Composite,
    }
}

/// A frame's 2D shots, in its order: what the 2D path draws.
pub(in crate::app) fn flat_shots(frame: &FrameShots) -> Vec<Shot> {
    frame
        .shots
        .iter()
        .filter_map(|s| match s.shot {
            AnyShot::D2(shot) => Some(shot),
            AnyShot::D3(_) => None,
        })
        .collect()
}

/// A frame's texture shots, in its order.
pub(in crate::app) fn texture_shots(frame: &FrameShots) -> Vec<FrameShot> {
    frame
        .shots
        .iter()
        .filter(|s| s.shot.target().is_some())
        .copied()
        .collect()
}

/// Everything drawing a frame with a 3D camera needs, borrowed for it.
pub(in crate::app) struct FrameDraw<'a> {
    /// The 2D shots' half, and the assets the HUD is made from.
    pub shots: ShotDraw<'a>,
    /// The 3D shots' half.
    pub world3d: &'a mut World3D,
    /// The asset root `default.render.ron` is read from.
    pub root: &'a Path,
    /// Physical pixels per point, for the HUD.
    pub scale: f32,
}

impl FrameDraw<'_> {
    /// Draws `frame`'s texture shots, and nothing else: the editor's 2D
    /// view shows what they fill.
    pub fn textures(self, frame: &FrameShots) {
        self.world3d.warn_cycles(frame);
        let shots = texture_shots(frame);
        if shots.is_empty() {
            return;
        }
        if shots.iter().any(|s| s.shot.is_3d()) {
            self.world3d.prepare(&mut *self.shots.renderer, self.root);
        }
        let mut draw = self.shots;
        draw.draw_frame(self.world3d, &shots, (1, 1));
    }

    /// Draws `frame` by `plan` into `output`, the window for `None`, of
    /// `size` pixels — texture shots first, the HUD last. A [`Plan::Flat`]
    /// frame is the 2D path's, and is drawn by it instead.
    pub fn draw(
        self,
        frame: &FrameShots,
        plan: Plan,
        output: Option<&RenderTarget>,
        size: (u32, u32),
    ) {
        let Self {
            shots: mut draw,
            world3d,
            root,
            scale,
        } = self;
        world3d.prepare(&mut *draw.renderer, root);
        world3d.warn_cycles(frame);
        let rects = match plan {
            Plan::Flat => {
                log::error!("a 2D frame was handed to the 3D frame's draw; not drawn (a bug)");
                return;
            }
            Plan::Lone3D(_) => {
                draw.draw_frame(world3d, &texture_shots(frame), size);
                Vec::new()
            }
            Plan::Composite => draw.draw_frame(world3d, &frame.shots, size),
        };
        let hud = shots::hud(
            draw.world,
            draw.atlases,
            draw.textures,
            draw.fonts,
            size,
            scale,
        );
        let device = draw.renderer.context().device().clone();
        let hud_mesh = hud.upload(&device);
        let white = draw.renderer.white_bind_group().clone();
        let hud_draws = mesh_draws(&hud, draw.textures, &white);
        let hud = Layer {
            mesh: hud_mesh.as_ref(),
            draws: &hud_draws,
        };
        let _render = voltra_profile::scope("render");
        match plan {
            Plan::Lone3D(shot) => world3d.draw(
                draw.world,
                draw.renderer,
                &shot,
                output,
                Picture3D::Opaque,
                hud,
            ),
            _ => {
                let placed = shots::placed(draw.pool, &rects);
                match output {
                    Some(target) => draw.renderer.composite(target, &placed, hud),
                    None => draw.renderer.present_composite(&placed, hud),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use voltra_ecs::World;
    use voltra_render::glam::Vec3;
    use voltra_scene::camera::frame_shots;
    use voltra_scene::{Camera2D, Camera3D, SceneId, Transform2D, Transform3D, Viewport};

    use super::*;

    fn spawn<C: 'static>(world: &mut World, camera: C, three: bool) {
        let e = world.spawn();
        world.insert(e, SceneId::new());
        if three {
            world.insert(e, Transform3D::from_translation(Vec3::Z));
        } else {
            world.insert(e, Transform2D::default());
        }
        world.insert(e, camera);
    }

    #[test]
    fn a_2d_frame_keeps_the_2d_path_and_a_lone_3d_camera_the_direct_one() {
        let mut world = World::new();
        spawn(&mut world, Camera2D::default(), false);
        assert_eq!(plan(&frame_shots(&world, (640, 360))), Plan::Flat);
        let mut world = World::new();
        spawn(&mut world, Camera3D::default(), true);
        assert!(matches!(
            plan(&frame_shots(&world, (640, 360))),
            Plan::Lone3D(_)
        ));
        // A part of the screen is a picture placed by the composite.
        let mut world = World::new();
        spawn(
            &mut world,
            Camera3D {
                viewport: Viewport::LEFT,
                ..Camera3D::default()
            },
            true,
        );
        assert_eq!(plan(&frame_shots(&world, (640, 360))), Plan::Composite);
    }

    #[test]
    fn a_3d_camera_and_a_2d_one_composite() {
        let mut world = World::new();
        spawn(&mut world, Camera3D::default(), true);
        spawn(&mut world, Camera2D::default(), false);
        let frame = frame_shots(&world, (640, 360));
        assert_eq!(plan(&frame), Plan::Composite);
        assert_eq!(flat_shots(&frame).len(), 1);
        assert!(texture_shots(&frame).is_empty());
    }
}
