//! The 3D camera component: which part of the 3D world a running game
//! sees — the twin of [`Camera2D`](crate::Camera2D).

mod projection;

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use voltra_ecs::{Entity, World};
use voltra_render::render3d::View3D;

pub use projection::Projection3D;

/// A 3D camera authored into the scene, looking down its entity's −Z.
///
/// Placed by the entity's [`Transform3D`](crate::Transform3D), composed
/// through its parents; its scale is read for nothing, as a
/// [`Camera2D`](crate::Camera2D)'s is not. An entity with a `Camera3D` and
/// no `Transform3D` sits in its parent's frame — the world origin at the
/// root. Godot's `Camera3D`, Unity's `Camera`, Bevy's `Camera3d`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Camera3D {
    /// How it projects: perspective by default.
    #[serde(default)]
    pub projection: Projection3D,
    /// Which camera wins when more than one is active: higher takes the
    /// frame, as [`Camera2D::priority`](crate::Camera2D::priority).
    #[serde(default)]
    pub priority: i32,
    /// Whether this camera is a candidate at all.
    #[serde(default = "yes")]
    pub active: bool,
    /// What the frame is cleared to before the world is drawn. Not written
    /// at its default.
    #[serde(default, skip_serializing_if = "CameraClear::is_background")]
    pub clear: CameraClear,
}

fn yes() -> bool {
    true
}

impl Default for Camera3D {
    fn default() -> Self {
        Self {
            projection: Projection3D::default(),
            priority: 0,
            active: true,
            clear: CameraClear::Background,
        }
    }
}

/// What a camera's picture starts as, before its world is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum CameraClear {
    /// The renderer's background colour.
    #[default]
    Background,
    /// This colour, linear RGBA.
    Color([f32; 4]),
}

impl CameraClear {
    /// Whether it is the default, [`Background`](Self::Background).
    pub fn is_background(&self) -> bool {
        *self == Self::Background
    }
}

impl Camera3D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Camera3D";

    /// Sets the draw order among cameras; see [`Camera3D::priority`].
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Sets how it projects.
    pub fn with_projection(mut self, projection: Projection3D) -> Self {
        self.projection = projection;
        self
    }
}

/// The 3D camera a game renders through: the highest-priority active one,
/// ties to the lowest entity index, as [`camera::active`](crate::camera::active).
pub fn active(world: &World) -> Option<Entity> {
    world
        .query::<Camera3D>()
        .filter(|(_, camera)| camera.active)
        .min_by_key(|(entity, camera)| (-camera.priority, entity.index()))
        .map(|(entity, _)| entity)
}

/// The view `entity`'s camera describes at `aspect`, or `None` if it
/// carries no [`Camera3D`]. A projection with a [`problem`](Projection3D::problem)
/// is drawn with that field made drawable, and warned about once per
/// entity and problem.
pub fn view(world: &World, entity: Entity, aspect: f32) -> Option<View3D> {
    let camera = world.get::<Camera3D>(entity)?;
    if let Some(problem) = camera.projection.problem() {
        warn_once(entity, problem);
    }
    let placement = crate::hierarchy::world_matrix_3d(world, entity);
    Some(camera.projection.view(placement, aspect))
}

/// [`active`], then [`view`]: `None` when no 3D camera is active.
pub fn active_view(world: &World, aspect: f32) -> Option<View3D> {
    view(world, active(world)?, aspect)
}

fn warn_once(entity: Entity, problem: &'static str) {
    static SEEN: OnceLock<Mutex<HashSet<(Entity, &'static str)>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    let first = seen
        .lock()
        .map_or(true, |mut seen| seen.insert((entity, problem)));
    if first {
        log::warn!("the Camera3D on {entity:?} cannot draw as set: {problem}");
    }
}

#[cfg(test)]
mod tests {
    use voltra_render::glam::{Vec3, Vec4};

    use super::*;
    use crate::{SceneId, Transform3D};

    fn camera(world: &mut World, camera: Camera3D, at: Vec3) -> Entity {
        let e = world.spawn();
        world.insert(e, SceneId::new());
        world.insert(e, Transform3D::from_translation(at));
        world.insert(e, camera);
        e
    }

    #[test]
    fn the_highest_active_camera_frames_the_game() {
        let mut world = World::new();
        camera(&mut world, Camera3D::default(), Vec3::ZERO);
        let top = camera(&mut world, Camera3D::default().with_priority(2), Vec3::X);
        camera(
            &mut world,
            Camera3D {
                active: false,
                ..Camera3D::default().with_priority(9)
            },
            Vec3::Y,
        );
        assert_eq!(active(&world), Some(top));
        let view = active_view(&world, 1.0).expect("a camera");
        assert!((view.eye - Vec3::X).length() < 1e-6);
    }

    #[test]
    fn a_camera_looks_down_its_minus_z_through_its_parents() {
        let mut world = World::new();
        let rig = world.spawn();
        world.insert(rig, SceneId::new());
        world.insert(
            rig,
            Transform3D::from_translation(Vec3::new(0.0, 5.0, 0.0)).with_rotation(
                voltra_render::glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            ),
        );
        let eye = camera(&mut world, Camera3D::default(), Vec3::new(0.0, 0.0, 2.0));
        crate::hierarchy::set_parent(&mut world, eye, rig).expect("3D under 3D");
        let view = view(&world, eye, 1.0).expect("a camera");
        // The rig turned a quarter left: its +Z is world +X, its −Z world −X.
        assert!(
            (view.eye - Vec3::new(2.0, 5.0, 0.0)).length() < 1e-5,
            "{}",
            view.eye
        );
        assert!((view.forward - Vec3::NEG_X).length() < 1e-5);
        let ahead = view.view_projection() * Vec4::new(-10.0, 5.0, 0.0, 1.0);
        assert!(
            ahead.x.abs() < 1e-4 && ahead.y.abs() < 1e-4,
            "centred: {ahead}"
        );
    }

    #[test]
    fn a_camera_round_trips_and_writes_only_what_it_changes() {
        let plain = ron::to_string(&Camera3D::default()).expect("serialises");
        assert!(!plain.contains("clear"), "{plain}");
        let back: Camera3D = ron::from_str("(projection: Perspective())").expect("defaults");
        assert_eq!(back, Camera3D::default());
        let ortho = Camera3D {
            clear: CameraClear::Color([0.0, 0.0, 0.0, 1.0]),
            ..Camera3D::default().with_projection(Projection3D::orthographic(8.0))
        };
        let text = ron::to_string(&ortho).expect("serialises");
        assert_eq!(ron::from_str::<Camera3D>(&text).expect("parses"), ortho);
    }

    #[test]
    fn a_broken_projection_says_what_is_wrong_and_still_draws() {
        let broken = Projection3D::Perspective {
            fov_y_degrees: 60.0,
            near: 0.0,
            far: Some(10.0),
        };
        assert!(broken.problem().is_some());
        assert!(Projection3D::default().problem().is_none());
        assert!(Projection3D::orthographic(0.0).problem().is_some());
        let view = broken.view(voltra_render::glam::Affine3A::IDENTITY, 1.5);
        assert!(view.view_projection().is_finite());
    }
}
