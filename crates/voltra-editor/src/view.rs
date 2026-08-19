//! Which camera the viewport image is rendered through.
//!
//! Unity, Unreal and Godot all show two pictures of one scene: a navigable
//! editor view, and the game as its own camera frames it. All three keep them
//! as separate windows, docked side by side. There is one viewport panel here,
//! so the two are a mode on it rather than two panels — the picture is the
//! same picture either way, and what changes is which camera the frame was
//! drawn with.
//!
//! The switch is manual and Play does not touch it. A gizmo drag during play is
//! deliberately allowed (see [`crate::panels::toolbar`]), and a Play that seized
//! the viewport would take that away from anyone who had the scene view up.

pub mod overlay;

use voltra_ecs::World;
use voltra_render::Camera2D;
use voltra_scene::camera;

/// Which camera the viewport shows the scene through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// The editor's own camera, driven by [`crate::camera::ViewportCamera`].
    #[default]
    Scene,
    /// The scene's active [`Camera`], as a running game would frame it.
    ///
    /// [`Camera`]: voltra_scene::Camera
    Game,
}

impl ViewMode {
    pub const ALL: [ViewMode; 2] = [ViewMode::Scene, ViewMode::Game];

    pub fn label(self) -> &'static str {
        match self {
            ViewMode::Scene => "Scene",
            ViewMode::Game => "Game",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            ViewMode::Scene => "Scene — navigate the world with the editor camera",
            ViewMode::Game => "Game — see the world through the scene's active camera",
        }
    }
}

/// The mode, and the editor camera parked while the game camera has the frame.
///
/// Parked rather than recomputed: an editor camera is somewhere the user put it
/// by panning and zooming, and losing that on a round trip through the game view
/// would make the toggle expensive to press.
#[derive(Debug, Default)]
pub struct View {
    mode: ViewMode,
    parked: Option<Camera2D>,
}

impl View {
    pub fn mode(&self) -> ViewMode {
        self.mode
    }

    pub fn is_game(&self) -> bool {
        self.mode == ViewMode::Game
    }

    /// Switches mode, parking or putting back the editor camera.
    ///
    /// Idempotent: setting the mode already in effect parks nothing, so two
    /// clicks on the same button cannot overwrite the parked camera with the
    /// game one.
    pub fn set_mode(&mut self, mode: ViewMode, camera: &mut Camera2D) {
        if mode == self.mode {
            return;
        }
        self.mode = mode;
        match mode {
            ViewMode::Game => self.parked = Some(*camera),
            ViewMode::Scene => {
                if let Some(parked) = self.parked.take() {
                    // Position and zoom, not the whole camera: `aspect` belongs
                    // to the target the frame is drawn into, which may have been
                    // resized while the game view had the viewport.
                    camera.position = parked.position;
                    camera.set_zoom(parked.zoom());
                }
            }
        }
    }

    /// Points `camera` at the scene's active camera, keeping its aspect.
    ///
    /// Returns whether there was one. `false` leaves `camera` untouched: the
    /// last framing on screen is more use than a black rectangle while the
    /// author works out which camera to enable, and the panel says so in words.
    pub fn show_game_camera(&self, world: &World, camera: &mut Camera2D) -> bool {
        let Some(entity) = camera::active(world) else {
            return false;
        };
        let Some(view) = camera::view(world, entity, camera.aspect) else {
            return false;
        };
        *camera = view;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_render::glam::Vec2;
    use voltra_scene::{Camera, Transform};

    fn editor_camera() -> Camera2D {
        Camera2D::new(Vec2::new(7.0, -3.0), 4.0, 1.5)
    }

    fn scene_with_camera(at: Vec2, size: f32) -> World {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform::from_translation(at));
        world.insert(entity, Camera::new(size));
        world
    }

    #[test]
    fn a_new_view_shows_the_scene() {
        let view = View::default();
        assert_eq!(view.mode(), ViewMode::Scene);
        assert!(!view.is_game());
    }

    #[test]
    fn going_to_the_game_view_and_back_puts_the_editor_camera_back() {
        let mut view = View::default();
        let mut camera = editor_camera();
        let before = camera;

        view.set_mode(ViewMode::Game, &mut camera);
        // The game camera writes over it while the mode is on.
        camera.position = Vec2::new(100.0, 100.0);
        camera.set_zoom(0.5);
        view.set_mode(ViewMode::Scene, &mut camera);

        assert_eq!(camera.position, before.position);
        assert_eq!(camera.zoom(), before.zoom());
    }

    #[test]
    fn a_resize_during_the_game_view_keeps_the_new_aspect() {
        let mut view = View::default();
        let mut camera = editor_camera();

        view.set_mode(ViewMode::Game, &mut camera);
        camera.aspect = 2.5;
        view.set_mode(ViewMode::Scene, &mut camera);

        assert_eq!(
            camera.aspect, 2.5,
            "the target's aspect is not the camera's to restore"
        );
    }

    #[test]
    fn setting_the_mode_already_in_effect_parks_nothing() {
        let mut view = View::default();
        let mut camera = editor_camera();
        let before = camera;

        view.set_mode(ViewMode::Game, &mut camera);
        camera.position = Vec2::new(50.0, 50.0);
        // A second click on the same button, which every toolbar allows.
        view.set_mode(ViewMode::Game, &mut camera);
        view.set_mode(ViewMode::Scene, &mut camera);

        assert_eq!(camera.position, before.position);
    }

    #[test]
    fn the_game_view_takes_the_scene_cameras_framing() {
        let view = View::default();
        let world = scene_with_camera(Vec2::new(2.0, 5.0), 3.0);
        let mut camera = editor_camera();

        assert!(view.show_game_camera(&world, &mut camera));

        assert_eq!(camera.position, Vec2::new(2.0, 5.0));
        assert!((camera.half_extents().y - 3.0).abs() < 1e-5);
        assert_eq!(
            camera.aspect, 1.5,
            "the viewport's aspect, not the camera's"
        );
    }

    #[test]
    fn a_scene_with_no_camera_leaves_the_viewport_alone() {
        let view = View::default();
        let world = World::new();
        let mut camera = editor_camera();
        let before = camera;

        assert!(!view.show_game_camera(&world, &mut camera));

        assert_eq!(camera, before);
    }

    #[test]
    fn an_inactive_camera_does_not_frame_the_game_view() {
        let view = View::default();
        let mut world = scene_with_camera(Vec2::new(2.0, 5.0), 3.0);
        for (_, camera) in world.query_mut::<Camera>() {
            camera.active = false;
        }
        let mut camera = editor_camera();
        let before = camera;

        assert!(!view.show_game_camera(&world, &mut camera));
        assert_eq!(camera, before);
    }
}
