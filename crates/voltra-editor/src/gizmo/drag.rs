//! A grab in progress.
//!
//! Both anchors are in **world** units, unlike [`Handle`], which is screen
//! pixels. That split is deliberate: the hit test has to match the picture, and
//! the movement has to survive the picture changing. A drag anchored in screen
//! space would make the sprite jump when the viewport is resized or the camera
//! zoomed mid-drag.

use voltra_ecs::Entity;
use voltra_render::glam::Vec2;

use super::handle::Handle;

/// The entity being moved, and where the movement started from.
#[derive(Debug, Clone, Copy)]
pub struct Drag {
    /// Held rather than re-picked each frame: a drag follows the entity it
    /// began on, even when the cursor leaves it or leaves the viewport.
    pub entity: Entity,
    pub handle: Handle,
    /// Cursor position, in world units, when the grab began.
    pub grab: Vec2,
    /// The entity's translation when the grab began.
    pub start: Vec2,
}

impl Drag {
    /// Where the entity belongs with the cursor at `cursor`, in world units.
    ///
    /// `start + (cursor − grab)`, not `cursor`. Setting the translation to the
    /// cursor teleports the sprite's origin under the pointer the moment a drag
    /// begins anywhere but dead centre — which is every drag, since the handles
    /// are drawn away from the origin.
    pub fn translation(&self, cursor: Vec2) -> Vec2 {
        self.start + self.handle.constrain(cursor - self.grab)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_ecs::World;

    /// A drag on a real entity.
    ///
    /// Spawned from a `World` because nothing public constructs an `Entity`
    /// from parts, and adding a constructor to `voltra-ecs` for a test's
    /// convenience would widen that crate's API for no other caller.
    fn drag(handle: Handle, grab: Vec2, start: Vec2) -> Drag {
        Drag {
            entity: World::new().spawn(),
            handle,
            grab,
            start,
        }
    }

    #[test]
    fn a_free_drag_moves_by_the_cursor_delta() {
        let d = drag(Handle::Both, Vec2::new(10.0, 10.0), Vec2::new(2.0, 3.0));

        assert_eq!(
            d.translation(Vec2::new(14.0, 17.0)),
            Vec2::new(2.0 + 4.0, 3.0 + 7.0)
        );
    }

    #[test]
    fn the_entity_does_not_teleport_to_the_cursor() {
        // Grabbing an arm a long way from the sprite's origin and moving one
        // unit must move the sprite one unit — not jump its origin under the
        // cursor. Every first implementation of a gizmo gets this wrong.
        let d = drag(Handle::Both, Vec2::new(50.0, 50.0), Vec2::ZERO);

        assert_eq!(d.translation(Vec2::new(51.0, 50.0)), Vec2::new(1.0, 0.0));
    }

    #[test]
    fn an_x_drag_leaves_y_exactly_alone() {
        let d = drag(Handle::X, Vec2::ZERO, Vec2::new(5.0, 5.0));

        assert_eq!(d.translation(Vec2::new(3.0, 99.0)), Vec2::new(8.0, 5.0));
    }

    #[test]
    fn a_y_drag_leaves_x_exactly_alone() {
        let d = drag(Handle::Y, Vec2::ZERO, Vec2::new(5.0, 5.0));

        assert_eq!(d.translation(Vec2::new(99.0, 3.0)), Vec2::new(5.0, 8.0));
    }

    #[test]
    fn a_drag_that_has_not_moved_changes_nothing() {
        let grab = Vec2::new(7.0, -2.0);
        let start = Vec2::new(1.0, 1.0);
        let d = drag(Handle::Both, grab, start);

        assert_eq!(d.translation(grab), start);
    }
}
