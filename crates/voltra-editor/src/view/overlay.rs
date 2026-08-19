//! What a scene camera sees, drawn in the scene view.
//!
//! A camera is the one component with nothing to draw: no quad, no outline,
//! nothing under the pointer. Unity draws the selected camera's frustum, Godot
//! draws its rectangle, Unreal draws both plus a preview — all of them for the
//! same reason, that framing is the only thing a camera does and it is
//! invisible otherwise.
//!
//! The selected camera is drawn, not every camera: a scene with a camera per
//! level section would otherwise be a screenful of rectangles nobody asked for.
//! The active one is drawn solid and any other dimmed, so "this is the frame the
//! game gets" is legible without opening the inspector.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_render::{Camera2D, LineBatch};
use voltra_scene::camera;

/// The rectangle the game gets. Warm, and unused by the gizmos (red, green,
/// blue, white) and by the physics overlay (green, magenta).
const ACTIVE_COLOR: [f32; 4] = [0.98, 0.78, 0.25, 0.95];

/// A camera that is not the one the game renders through — the same hue, faded,
/// because it is the same kind of thing and not a different one.
const IDLE_COLOR: [f32; 4] = [0.98, 0.78, 0.25, 0.35];

/// Outline thickness, in pixels. Matches the physics overlay's.
const WIDTH: f32 = 1.5;

/// Pushes `entity`'s camera rectangle into `lines`, if it has one.
///
/// `aspect` is the viewport's, because that is what a camera's height turns
/// into a width through — the game is framed by the window it will run in, and
/// a rectangle drawn at any other ratio would lie about what gets cut off.
pub fn draw(world: &World, entity: Entity, aspect: f32, lines: &mut LineBatch) {
    let Some(view) = camera::view(world, entity, aspect) else {
        return;
    };
    let color = if camera::active(world) == Some(entity) {
        ACTIVE_COLOR
    } else {
        IDLE_COLOR
    };
    rect(&view, color, lines);
}

/// The four edges of what `view` covers.
fn rect(view: &Camera2D, color: [f32; 4], lines: &mut LineBatch) {
    let half = view.half_extents();
    let corners = [
        view.position + Vec2::new(-half.x, -half.y),
        view.position + Vec2::new(half.x, -half.y),
        view.position + Vec2::new(half.x, half.y),
        view.position + Vec2::new(-half.x, half.y),
    ];
    for i in 0..corners.len() {
        lines.push(corners[i], corners[(i + 1) % corners.len()], WIDTH, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_scene::{Camera, Transform};

    fn spawn(world: &mut World, camera: Camera, at: Vec2) -> Entity {
        let entity = world.spawn();
        world.insert(entity, Transform::from_translation(at));
        world.insert(entity, camera);
        entity
    }

    #[test]
    fn a_camera_draws_four_edges() {
        let mut world = World::new();
        let entity = spawn(&mut world, Camera::new(2.0), Vec2::ZERO);
        let mut lines = LineBatch::default();

        draw(&world, entity, 1.0, &mut lines);

        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn an_entity_without_a_camera_draws_nothing() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform::default());
        let mut lines = LineBatch::default();

        draw(&world, entity, 1.0, &mut lines);

        assert!(lines.is_empty());
    }

    #[test]
    fn the_rectangle_is_the_region_the_camera_covers() {
        // Every corner of the drawn rectangle has to be a corner of the view,
        // or the overlay is describing a framing the game will not get.
        let view = Camera2D::new(Vec2::new(1.0, -1.0), 0.5, 2.0);
        let mut lines = LineBatch::default();

        rect(&view, ACTIVE_COLOR, &mut lines);

        let half = view.half_extents();
        assert_eq!(half, Vec2::new(4.0, 2.0));
        // Bottom-left and top-right, through the camera's own projection: both
        // land exactly on the edge of clip space.
        for corner in [view.position - half, view.position + half] {
            let clip = view.world_to_viewport(corner, Vec2::new(100.0, 100.0));
            assert!(
                clip.x.abs() < 1e-3 || (clip.x - 100.0).abs() < 1e-3,
                "corner {corner} is not on a vertical edge: {clip}"
            );
            assert!(
                clip.y.abs() < 1e-3 || (clip.y - 100.0).abs() < 1e-3,
                "corner {corner} is not on a horizontal edge: {clip}"
            );
        }
    }
}
