//! The camera component: which part of the world a running game sees.
//!
//! The editor's own camera is not this. That one belongs to the tool and lives
//! in `voltra-editor::camera::ViewportCamera`; this one is authored, saved with
//! the scene and is what a shipped game renders through. Unity, Godot, Unreal
//! and Bevy all draw the same line — a scene camera is a component (or node,
//! or actor) in the world, and the editor's viewport camera is not in the world
//! at all.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_render::Camera2D;

use crate::hierarchy::world_matrix;

/// A camera authored into the scene.
///
/// Its position comes from the entity's [`Transform`], composed through any
/// parents, so a camera can be hung off the thing it follows without a follow
/// system existing yet. An entity with a `Camera` and no `Transform` sits at
/// the world origin, which is what the identity matrix already means
/// everywhere else.
///
/// [`Transform`]: crate::transform::Transform
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Camera {
    /// Half the world-space height the camera shows.
    ///
    /// Unity's `orthographicSize`, and stored the way Unity stores it rather
    /// than as [`Camera2D::zoom`]'s reciprocal: "this camera sees five units
    /// of world" is a sentence about the scene, while a zoom factor is a
    /// sentence about a viewport. `view` converts.
    pub size: f32,
    /// Which camera wins when more than one is active. Higher takes the frame.
    ///
    /// Unity's `depth` and Bevy's `order`. An `i32` rather than an `f32` so
    /// ties are exact, the same reason [`Sprite::sort_order`] is one.
    ///
    /// [`Sprite::sort_order`]: crate::sprite::Sprite::sort_order
    pub priority: i32,
    /// Whether this camera is a candidate at all.
    ///
    /// Godot's `enabled`, Bevy's `is_active`. A scene usually holds several
    /// framings and renders through one; deleting the others to choose is not
    /// a choice anyone should have to make.
    pub active: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            // One world unit above and below the centre, which is exactly what
            // `Camera2D::default` shows. A camera dropped into a scene frames
            // what the editor was already framing at its home zoom.
            size: 1.0,
            priority: 0,
            active: true,
        }
    }
}

impl Camera {
    /// Bounds on [`Self::size`], mirroring [`Camera2D`]'s on zoom.
    ///
    /// The reciprocal of the pair over there, because that is the same
    /// constraint written in this type's units: `Camera2D::set_zoom` clamps
    /// whatever `view` hands it, so these exist to say what the field means,
    /// and so a UI can stop a drag at the wall rather than at infinity.
    pub const MIN_SIZE: f32 = 1.0 / Camera2D::MAX_ZOOM;
    pub const MAX_SIZE: f32 = 1.0 / Camera2D::MIN_ZOOM;

    pub fn new(size: f32) -> Self {
        Self {
            size,
            ..Default::default()
        }
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// The zoom [`Camera2D`] wants for this size.
    ///
    /// A zero or negative size produces an infinity, and a `NaN` size produces
    /// a `NaN`. Both are handed straight to `Camera2D::set_zoom`, which clamps
    /// the first and refuses the second — guarding again here would mean two
    /// places deciding what a broken camera does.
    pub fn zoom(&self) -> f32 {
        1.0 / self.size
    }
}

/// The camera a game renders through: the highest-priority active one.
///
/// `None` when the scene holds no active camera, which is a state to report
/// rather than to paper over — Unity's Game view says "No cameras rendering"
/// instead of inventing a framing, because a scene that draws nothing has a
/// cause the author can fix.
///
/// Ties break on [`Entity::index`], ascending, so the answer does not depend on
/// query order. Priority is the control for when the tie matters.
pub fn active(world: &World) -> Option<Entity> {
    world
        .query::<Camera>()
        .filter(|(_, camera)| camera.active)
        .min_by_key(|(entity, camera)| (-camera.priority, entity.index()))
        .map(|(entity, _)| entity)
}

/// The view `entity`'s camera describes, at `aspect`.
///
/// `None` if it carries no [`Camera`]. Rotation and scale in the transform are
/// read for nothing: [`Camera2D`] is a position and a zoom, so a turned camera
/// would need a rotated view matrix and a rotated `viewport_to_world` behind
/// it, which is a stage of its own rather than a field.
pub fn view(world: &World, entity: Entity, aspect: f32) -> Option<Camera2D> {
    let camera = *world.get::<Camera>(entity)?;
    let position = world_matrix(world, entity).transform_point2(Vec2::ZERO);
    Some(Camera2D::new(position, camera.zoom(), aspect))
}

/// The view the scene renders through at `aspect`: [`active`], then [`view`].
///
/// `None` when no camera is active, which is the state both callers have to
/// answer for — the editor's game view keeps the last framing and says so in
/// words, and the player keeps the default framing and logs it once. They
/// answer it differently, so this composes the two lookups and nothing more.
pub fn active_view(world: &World, aspect: f32) -> Option<Camera2D> {
    view(world, active(world)?, aspect)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy::link::set_parent;
    use crate::scene_id::SceneId;
    use crate::transform::Transform;

    fn spawn(world: &mut World, camera: Camera, at: Vec2) -> Entity {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(entity, Transform::from_translation(at));
        world.insert(entity, camera);
        entity
    }

    #[test]
    fn a_scene_without_a_camera_has_no_active_one() {
        let mut world = World::new();
        world.spawn();
        assert_eq!(active(&world), None);
    }

    #[test]
    fn the_only_camera_is_the_active_one() {
        let mut world = World::new();
        let camera = spawn(&mut world, Camera::default(), Vec2::ZERO);
        assert_eq!(active(&world), Some(camera));
    }

    #[test]
    fn the_highest_priority_wins() {
        let mut world = World::new();
        spawn(&mut world, Camera::default(), Vec2::ZERO);
        let winner = spawn(
            &mut world,
            Camera::default().with_priority(5),
            Vec2::new(3.0, 0.0),
        );
        spawn(&mut world, Camera::default().with_priority(-1), Vec2::ZERO);

        assert_eq!(active(&world), Some(winner));
    }

    #[test]
    fn an_inactive_camera_is_never_chosen() {
        let mut world = World::new();
        let low = spawn(&mut world, Camera::default(), Vec2::ZERO);
        spawn(
            &mut world,
            Camera {
                active: false,
                ..Camera::default().with_priority(100)
            },
            Vec2::ZERO,
        );

        assert_eq!(active(&world), Some(low));
    }

    #[test]
    fn a_tie_breaks_on_the_lower_entity_index() {
        let mut world = World::new();
        let first = spawn(&mut world, Camera::default(), Vec2::ZERO);
        let second = spawn(&mut world, Camera::default(), Vec2::ZERO);

        assert!(first.index() < second.index());
        assert_eq!(
            active(&world),
            Some(first),
            "ties must not depend on query order"
        );
    }

    #[test]
    fn the_view_sits_where_the_transform_says() {
        let mut world = World::new();
        let entity = spawn(&mut world, Camera::default(), Vec2::new(3.0, -4.0));

        let view = view(&world, entity, 1.5).expect("it has a camera");
        assert_eq!(view.position, Vec2::new(3.0, -4.0));
        assert_eq!(view.aspect, 1.5);
    }

    #[test]
    fn a_size_is_half_the_world_height_the_view_covers() {
        let mut world = World::new();
        let entity = spawn(&mut world, Camera::new(5.0), Vec2::ZERO);

        let view = view(&world, entity, 2.0).expect("it has a camera");
        assert!((view.half_extents().y - 5.0).abs() < 1e-5);
        assert!(
            (view.half_extents().x - 10.0).abs() < 1e-5,
            "aspect widens x"
        );
    }

    #[test]
    fn a_parented_camera_is_where_its_parent_put_it() {
        let mut world = World::new();
        let rig = spawn(&mut world, Camera::default(), Vec2::new(10.0, 0.0));
        let entity = spawn(&mut world, Camera::default(), Vec2::new(1.0, 2.0));
        set_parent(&mut world, entity, rig).expect("a plain reparent");

        let view = view(&world, entity, 1.0).expect("it has a camera");
        assert!((view.position - Vec2::new(11.0, 2.0)).length() < 1e-5);
    }

    #[test]
    fn a_camera_without_a_transform_sits_at_the_origin() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Camera::default());

        let view = view(&world, entity, 1.0).expect("it has a camera");
        assert_eq!(view.position, Vec2::ZERO);
    }

    #[test]
    fn an_entity_without_a_camera_has_no_view() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform::default());
        assert!(view(&world, entity, 1.0).is_none());
    }

    #[test]
    fn a_degenerate_size_still_produces_a_finite_view() {
        // A size dragged to zero, or typed as one into the inspector. The
        // clamp lives in `Camera2D::set_zoom`; this is the test that the path
        // through this type actually reaches it.
        let mut world = World::new();
        let entity = spawn(&mut world, Camera::new(0.0), Vec2::ZERO);

        let view = view(&world, entity, 1.0).expect("it has a camera");
        assert_eq!(view.zoom(), Camera2D::MAX_ZOOM);
        assert!(view.half_extents().is_finite());
    }

    #[test]
    fn a_nan_size_leaves_a_usable_view() {
        let mut world = World::new();
        let entity = spawn(&mut world, Camera::new(f32::NAN), Vec2::ZERO);

        let view = view(&world, entity, 1.0).expect("it has a camera");
        assert!(
            view.half_extents().is_finite(),
            "a NaN must not poison the frame"
        );
    }

    #[test]
    fn the_size_bounds_are_the_zoom_bounds_the_other_way_up() {
        assert!((Camera::new(Camera::MAX_SIZE).zoom() - Camera2D::MIN_ZOOM).abs() < 1e-9);
        assert!((Camera::new(Camera::MIN_SIZE).zoom() - Camera2D::MAX_ZOOM).abs() < 1e-3);
    }

    #[test]
    fn the_active_view_frames_the_camera_that_wins() {
        let mut world = World::new();
        spawn(&mut world, Camera::new(2.0), Vec2::new(-5.0, 0.0));
        spawn(
            &mut world,
            Camera::new(4.0).with_priority(1),
            Vec2::new(7.0, -3.0),
        );

        let view = active_view(&world, 1.5).expect("one of them is active");
        assert_eq!(view.position, Vec2::new(7.0, -3.0));
        assert_eq!(view.zoom(), 0.25);
        assert_eq!(view.aspect, 1.5);
    }

    #[test]
    fn a_scene_without_an_active_camera_has_no_active_view() {
        let mut world = World::new();
        spawn(
            &mut world,
            Camera {
                active: false,
                ..Default::default()
            },
            Vec2::ZERO,
        );

        assert!(active_view(&world, 1.0).is_none());
    }
}
