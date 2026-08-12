//! The translate gizmo: what is drawn over the selection, and what a drag does.
//!
//! Split three ways because the three parts fail differently. [`handle`] is
//! screen-space geometry, [`drag`] is world-space arithmetic, and this file is
//! the frame around them: it reads the pointer, decides whether a press begins
//! a drag, and asks the world for the transform. The first two are pure and
//! unit-tested; this one needs egui and a world, and is verified by driving the
//! editor.

pub mod drag;
pub mod handle;

use voltra_core::egui::{PointerButton, Response};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::Transform;

use drag::Drag;
use handle::{Handle, AXIS_LENGTH, CENTRE_HALF, LINE_WIDTH};

/// x red, y green — the axis convention every editor shares.
const X_COLOR: [f32; 4] = [0.90, 0.25, 0.25, 1.0];
const Y_COLOR: [f32; 4] = [0.35, 0.85, 0.35, 1.0];
const CENTRE_COLOR: [f32; 4] = [0.95, 0.95, 0.95, 1.0];

/// The translate gizmo's state between frames.
#[derive(Debug, Default)]
pub struct Gizmo {
    /// `Some` only between a press on a handle and the release that ends it.
    drag: Option<Drag>,
}

impl Gizmo {
    /// Abandons any grab in progress, leaving the entity where it is.
    ///
    /// For when the world changes under the drag rather than the pointer
    /// ending it: a play-mode Stop despawns and respawns every entity, so the
    /// `Entity` and the grab offset a [`Drag`] holds are both stale.
    pub fn cancel_drag(&mut self) {
        self.drag = None;
    }

    /// Applies one frame of interaction, returning whether it consumed the
    /// pointer.
    ///
    /// The caller uses that to decide whether the click was also a selection. A
    /// handle drawn on top of a sprite has to win, or the gizmo becomes
    /// unusable exactly when the sprite fills the viewport.
    pub fn update(
        &mut self,
        response: &Response,
        frame: &mut UiFrame<'_>,
        selected: Option<Entity>,
    ) -> bool {
        let viewport = viewport_size(response);
        if viewport.x <= 0.0 || viewport.y <= 0.0 {
            // A minimised window. Every conversion below divides by this.
            self.drag = None;
            return false;
        }

        // A release ends the drag wherever it happens, including outside the
        // viewport: the drag belongs to the gizmo, not to the cursor still
        // being over a handle.
        if self.drag.is_some() && !response.is_pointer_button_down_on() {
            self.drag = None;
            return true;
        }

        let Some(pointer) = response.interact_pointer_pos().or(response.hover_pos()) else {
            return self.drag.is_some();
        };
        // Global screen points; the camera works in viewport-local ones, so the
        // panel's own corner comes off first.
        let local = Vec2::new(
            pointer.x - response.rect.min.x,
            pointer.y - response.rect.min.y,
        );
        let cursor_world = frame.camera.viewport_to_world(local, viewport);

        if let Some(active) = self.drag {
            let translation = active.translation(cursor_world);
            let Some(transform) = frame.world.get_mut::<Transform>(active.entity) else {
                // Despawned mid-drag, or its Transform removed. Ending the drag
                // is the whole response: there is nothing left to move.
                self.drag = None;
                return true;
            };
            transform.translation = translation;
            return true;
        }

        if !response.drag_started_by(PointerButton::Primary) {
            return false;
        }

        let Some(entity) = selected else {
            return false;
        };
        let Some(transform) = frame.world.get::<Transform>(entity) else {
            return false;
        };
        let start = transform.translation;
        let origin = frame.camera.world_to_viewport(start, viewport);
        let Some(handle) = Handle::at(local, origin) else {
            return false;
        };

        self.drag = Some(Drag {
            entity,
            handle,
            grab: cursor_world,
            start,
        });
        true
    }

    /// Pushes this frame's segments for the selection, if there is one.
    pub fn draw(&self, response: &Response, frame: &mut UiFrame<'_>, selected: Option<Entity>) {
        let viewport = viewport_size(response);
        if viewport.x <= 0.0 || viewport.y <= 0.0 {
            return;
        }
        let Some(entity) = selected else {
            return;
        };
        let Some(transform) = frame.world.get::<Transform>(entity) else {
            return;
        };

        // Laid out before `lines()` is called: that borrows the frame mutably,
        // and the layout needs the camera off the same frame.
        let segments = segments(frame.camera, viewport, transform.translation);

        let lines = frame.lines();
        for (a, b, color) in segments {
            lines.push(a, b, LINE_WIDTH, color);
        }
    }
}

/// The gizmo's six segments, in world units, for an origin at `origin`.
///
/// Pure, and separated from [`Gizmo::draw`] for exactly that: the property that
/// matters — an arm the same length on screen at every zoom — is a statement
/// about these numbers, and testing it through egui and a GPU would be testing
/// it nowhere.
///
/// The arms are a length in *pixels*, so they are laid out in screen space and
/// converted back. Any other order gives arms that grow and shrink with the
/// camera.
fn segments(
    camera: &voltra_render::Camera2D,
    viewport: Vec2,
    origin: Vec2,
) -> [(Vec2, Vec2, [f32; 4]); 6] {
    let screen = camera.world_to_viewport(origin, viewport);
    let to_world = |offset: Vec2| camera.viewport_to_world(screen + offset, viewport);

    let x_tip = to_world(Vec2::new(AXIS_LENGTH, 0.0));
    // Screen y grows downward, so the arm drawn upward is negative.
    let y_tip = to_world(Vec2::new(0.0, -AXIS_LENGTH));

    let half = (to_world(Vec2::splat(CENTRE_HALF)) - origin).abs();
    let (lo, hi) = (origin - half, origin + half);

    [
        (origin, x_tip, X_COLOR),
        (origin, y_tip, Y_COLOR),
        // The centre square as four segments. A filled quad would need the
        // sprite pipeline and a texture; four lines need nothing new.
        (Vec2::new(lo.x, lo.y), Vec2::new(hi.x, lo.y), CENTRE_COLOR),
        (Vec2::new(hi.x, lo.y), Vec2::new(hi.x, hi.y), CENTRE_COLOR),
        (Vec2::new(hi.x, hi.y), Vec2::new(lo.x, hi.y), CENTRE_COLOR),
        (Vec2::new(lo.x, hi.y), Vec2::new(lo.x, lo.y), CENTRE_COLOR),
    ]
}

fn viewport_size(response: &Response) -> Vec2 {
    Vec2::new(response.rect.width(), response.rect.height())
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_render::Camera2D;

    const VIEWPORT: Vec2 = Vec2::new(800.0, 600.0);

    /// How long a segment is once projected back to the screen.
    fn screen_length(camera: &Camera2D, a: Vec2, b: Vec2) -> f32 {
        let sa = camera.world_to_viewport(a, VIEWPORT);
        let sb = camera.world_to_viewport(b, VIEWPORT);
        sa.distance(sb)
    }

    #[test]
    fn cancelling_ends_the_drag() {
        // What Stop needs: `Drag` holds an `Entity` and a grab offset, and both
        // address a world that a restore is about to despawn and respawn.
        let mut world = voltra_ecs::World::new();
        let entity = world.spawn();
        let mut gizmo = Gizmo {
            drag: Some(Drag {
                entity,
                handle: Handle::Both,
                grab: Vec2::ZERO,
                start: Vec2::ZERO,
            }),
        };

        gizmo.cancel_drag();

        assert!(gizmo.drag.is_none());
    }

    #[test]
    fn the_arms_are_the_declared_pixel_length() {
        let camera = Camera2D::new(Vec2::ZERO, 1.0, VIEWPORT.x / VIEWPORT.y);
        let [x, y, ..] = segments(&camera, VIEWPORT, Vec2::ZERO);

        assert!((screen_length(&camera, x.0, x.1) - AXIS_LENGTH).abs() < 0.01);
        assert!((screen_length(&camera, y.0, y.1) - AXIS_LENGTH).abs() < 0.01);
    }

    #[test]
    fn the_arms_keep_their_pixel_length_at_any_zoom() {
        // The whole reason the layout happens in screen space. A gizmo laid out
        // in world units doubles on screen when the camera zooms in.
        for zoom in [0.1, 0.5, 1.0, 4.0, 25.0] {
            let camera = Camera2D::new(Vec2::new(3.0, -2.0), zoom, VIEWPORT.x / VIEWPORT.y);
            let [x, ..] = segments(&camera, VIEWPORT, Vec2::new(1.0, 1.0));

            let length = screen_length(&camera, x.0, x.1);
            assert!(
                (length - AXIS_LENGTH).abs() < 0.01,
                "at zoom {zoom} the arm was {length} px, not {AXIS_LENGTH}"
            );
        }
    }

    #[test]
    fn the_x_arm_runs_right_and_the_y_arm_runs_up_on_screen() {
        let camera = Camera2D::new(Vec2::ZERO, 1.0, VIEWPORT.x / VIEWPORT.y);
        let [x, y, ..] = segments(&camera, VIEWPORT, Vec2::ZERO);

        let origin = camera.world_to_viewport(Vec2::ZERO, VIEWPORT);
        let x_tip = camera.world_to_viewport(x.1, VIEWPORT);
        let y_tip = camera.world_to_viewport(y.1, VIEWPORT);

        assert!(x_tip.x > origin.x, "the x arm must run right on screen");
        // Screen y grows downward, so up is a smaller y.
        assert!(y_tip.y < origin.y, "the y arm must run up on screen");
    }

    #[test]
    fn the_arms_start_at_the_selection() {
        let camera = Camera2D::new(Vec2::new(-4.0, 7.0), 2.0, VIEWPORT.x / VIEWPORT.y);
        let origin = Vec2::new(2.5, -1.5);
        let [x, y, ..] = segments(&camera, VIEWPORT, origin);

        assert_eq!(x.0, origin);
        assert_eq!(y.0, origin);
    }

    #[test]
    fn the_centre_square_closes_on_itself() {
        let camera = Camera2D::new(Vec2::ZERO, 1.0, VIEWPORT.x / VIEWPORT.y);
        let [_, _, a, b, c, d] = segments(&camera, VIEWPORT, Vec2::ZERO);

        // Each side starts where the previous one ended, and the last returns
        // to the first. An open square is four strokes that look like three.
        assert_eq!(a.1, b.0);
        assert_eq!(b.1, c.0);
        assert_eq!(c.1, d.0);
        assert_eq!(d.1, a.0);
    }

    #[test]
    fn the_centre_square_is_the_declared_pixel_size() {
        let camera = Camera2D::new(Vec2::ZERO, 3.0, VIEWPORT.x / VIEWPORT.y);
        let [_, _, side, ..] = segments(&camera, VIEWPORT, Vec2::ZERO);

        // Laid out from the centre, so a side spans twice the half-extent.
        let length = screen_length(&camera, side.0, side.1);
        assert!(
            (length - CENTRE_HALF * 2.0).abs() < 0.01,
            "a side was {length} px, not {}",
            CENTRE_HALF * 2.0
        );
    }
}
