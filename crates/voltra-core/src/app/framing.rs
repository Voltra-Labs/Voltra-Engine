//! Which camera a frame with no editor in it is drawn through.
//!
//! The editor answers this in `voltra-editor::view`, because it has a panel
//! to put the answer in and a second camera — its own — to fall back to. A
//! shipped game has neither, so the platform layer applies the scene's own
//! camera itself: without this the window would draw through
//! [`Camera2D::default`] forever and a camera authored into the scene would be
//! a component nothing reads once the editor is gone.

use voltra_ecs::World;
use voltra_render::Camera2D;

/// Applies the scene's active camera to the frame, and complains once when
/// there is none.
#[derive(Default)]
pub(super) struct SceneFraming {
    /// Whether the missing-camera line has already been logged.
    ///
    /// Latched rather than logged per frame: this runs sixty times a second,
    /// and a message repeated that often buries every other line in the log
    /// while saying nothing new. Cleared as soon as a camera answers, so a
    /// scene that loses its camera again says so again.
    warned: bool,
}

impl SceneFraming {
    /// Points `camera` at the scene's active camera, keeping its aspect.
    ///
    /// With no active camera the framing is left alone. Unity draws "No
    /// cameras rendering" into the Game view; there is no view to draw into
    /// here, so the log carries the same sentence and the window keeps the
    /// default framing rather than going black — a picture with a cause named
    /// in the log is more use than an empty one.
    pub(super) fn apply(&mut self, world: &World, camera: &mut Camera2D) {
        let Some(view) = voltra_scene::camera::active_view(world, camera.aspect) else {
            if !self.warned {
                log::warn!("no active camera in the scene: drawing through the default view");
                self.warned = true;
            }
            return;
        };

        *camera = view;
        self.warned = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_render::glam::Vec2;
    use voltra_scene::{Camera, Transform};

    fn world_with_camera(camera: Camera, at: Vec2) -> World {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform::from_translation(at));
        world.insert(entity, camera);
        world
    }

    #[test]
    fn the_scene_camera_takes_the_frame() {
        let world = world_with_camera(Camera::new(4.0), Vec2::new(3.0, -2.0));
        let mut camera = Camera2D::new(Vec2::ZERO, 1.0, 1.5);

        SceneFraming::default().apply(&world, &mut camera);

        assert_eq!(camera.position, Vec2::new(3.0, -2.0));
        assert_eq!(camera.zoom(), 0.25);
    }

    #[test]
    fn the_aspect_of_the_window_survives_the_framing() {
        // The camera stores a size, not a shape: how wide that is belongs to
        // the window, and a resize must not be undone by the next frame.
        let world = world_with_camera(Camera::new(2.0), Vec2::ZERO);
        let mut camera = Camera2D::new(Vec2::ZERO, 1.0, 2.25);

        SceneFraming::default().apply(&world, &mut camera);

        assert_eq!(camera.aspect, 2.25);
    }

    #[test]
    fn no_active_camera_leaves_the_framing_alone() {
        let world = world_with_camera(
            Camera {
                active: false,
                ..Default::default()
            },
            Vec2::new(9.0, 9.0),
        );
        let before = Camera2D::new(Vec2::new(1.0, 2.0), 3.0, 1.5);
        let mut camera = before;

        SceneFraming::default().apply(&world, &mut camera);

        assert_eq!(camera.position, before.position);
        assert_eq!(camera.zoom(), before.zoom());
    }

    #[test]
    fn the_complaint_is_latched_and_cleared_by_a_camera() {
        let mut framing = SceneFraming::default();
        let mut camera = Camera2D::default();

        framing.apply(&World::new(), &mut camera);
        assert!(framing.warned, "the first frame without a camera complains");

        framing.apply(&World::new(), &mut camera);
        assert!(framing.warned, "the second one does not complain again");

        let world = world_with_camera(Camera::default(), Vec2::ZERO);
        framing.apply(&world, &mut camera);
        assert!(
            !framing.warned,
            "a camera arriving must re-arm the complaint"
        );
    }
}
