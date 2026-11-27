//! Moving the scene's following and shaking cameras once per frame.

use voltra_scene::camera_rig;

use crate::app::App;

impl App {
    /// Runs every `CameraFollow2D` and `CameraShake` by the frame's delta.
    ///
    /// Per frame and gated on the simulation, as the animations are: a
    /// camera follows what is drawn, and an editor that is authoring does
    /// not have its scene camera chase the entity being placed. After the
    /// tick and the animations, so it follows where they left the target,
    /// and before the audio, whose ears ride on the camera (on its transform: a
    /// shake moves the view, not the ears).
    pub(super) fn update_camera_rigs(&mut self, delta: f32) {
        if !self.simulation.is_running() {
            return;
        }
        // The window's shape — square before there is a window — and the
        // render textures, for how wide each camera's own view is.
        let aspect = self
            .renderer
            .as_ref()
            .map_or(1.0, |renderer| renderer.camera.aspect);
        camera_rig::update(&mut self.world, delta, aspect, self.textures.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_render::glam::Vec2;
    use voltra_scene::{CameraFollow2D, SceneId, Transform2D};

    #[test]
    fn a_camera_follows_only_while_the_game_runs() {
        let mut app = App::default();
        let player = app.world.spawn();
        let id = SceneId::new();
        app.world.insert(player, id);
        app.world
            .insert(player, Transform2D::from_translation(Vec2::new(4.0, 0.0)));
        let camera = app.world.spawn();
        app.world.insert(camera, Transform2D::default());
        app.world.insert(camera, CameraFollow2D::new(id));
        let x = |app: &App| {
            app.world
                .get::<Transform2D>(camera)
                .expect("t")
                .translation
                .x
        };

        app.update_camera_rigs(0.1);
        assert_eq!(x(&app), 0.0, "authoring holds still");
        app.set_simulating(true);
        app.update_camera_rigs(0.1);
        assert_eq!(x(&app), 4.0, "the first frame lands on it");
    }
}
