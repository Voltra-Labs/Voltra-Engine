//! A grab in progress, and the transform it produces.
//!
//! The anchors are in **world** units, unlike [`Handle`], which is screen
//! pixels. That split is deliberate: the hit test has to match the picture, and
//! the movement has to survive the picture changing. A drag anchored in screen
//! space would make the sprite jump when the viewport is resized or the camera
//! zoomed mid-drag.
//!
//! Every tool reads the transform the entity had when the grab began and never
//! the one it has now. Deriving frame N from frame N−1 accumulates the rounding
//! of every frame in between, and a rotate that fights a scale for the same
//! floats drifts visibly within one drag.

use std::f32::consts::{PI, TAU};

use voltra_ecs::Entity;
use voltra_render::glam::{Mat2, Vec2};
use voltra_scene::Transform;

use super::handle::Handle;
use crate::tool::Tool;

/// Below this the grab offset is treated as having no direction.
///
/// World units, and small: the handles are drawn 60 pixels out, so a real grab
/// is never this close to the origin at any sane zoom. It guards the divisions
/// and the `atan2` against a cursor sitting exactly on the pivot.
const MIN_OFFSET: f32 = 1e-4;

/// How near zero a scale component may get.
///
/// Signed, so a flip through zero still flips. A scale that reaches exactly
/// zero is a one-way door — every later factor multiplies it and stays zero —
/// and the gizmo would have no way back out of a state it created.
const MIN_SCALE: f32 = 1e-3;

/// The entity being transformed, and everything the drag started from.
#[derive(Debug, Clone, Copy)]
pub struct Drag {
    /// Held rather than re-picked each frame: a drag follows the entity it
    /// began on, even when the cursor leaves it or leaves the viewport.
    pub entity: Entity,
    /// Held for the same reason: pressing `E` mid-drag switches the tool for
    /// the *next* grab, it does not turn a move already under way into a
    /// rotation.
    pub tool: Tool,
    pub handle: Handle,
    /// Cursor position, in world units, when the grab began.
    pub grab: Vec2,
    /// The entity's transform when the grab began.
    pub start: Transform,
    /// Angle from the pivot to the cursor last frame, in radians.
    last_angle: f32,
    /// Total angle turned since the grab began, in radians.
    ///
    /// Accumulated rather than measured, so a drag that carries the cursor past
    /// half a turn keeps going instead of snapping back the short way round —
    /// the same reason Blender and Unity track a running total.
    turned: f32,
}

impl Drag {
    /// Begins a grab on `entity`, whose transform is `start`.
    pub fn begin(entity: Entity, tool: Tool, handle: Handle, grab: Vec2, start: Transform) -> Self {
        Self {
            entity,
            tool,
            handle,
            grab,
            start,
            last_angle: angle(grab - start.translation).unwrap_or(0.0),
            turned: 0.0,
        }
    }

    /// The transform the entity belongs at with the cursor at `cursor`.
    ///
    /// Takes `&mut self` for the rotate tool alone, which has to remember where
    /// the cursor was last frame to know which way it went round.
    pub fn transform(&mut self, cursor: Vec2) -> Transform {
        match self.tool {
            Tool::Translate => Transform {
                translation: self.translation(cursor),
                ..self.start
            },
            Tool::Rotate => Transform {
                rotation: self.start.rotation + self.turn(cursor),
                ..self.start
            },
            Tool::Scale => Transform {
                scale: self.scale(cursor),
                ..self.start
            },
        }
    }

    /// Where the entity belongs with the cursor at `cursor`, in world units.
    ///
    /// `start + (cursor − grab)`, not `cursor`. Setting the translation to the
    /// cursor teleports the sprite's origin under the pointer the moment a drag
    /// begins anywhere but dead centre — which is every drag, since the handles
    /// are drawn away from the origin.
    pub fn translation(&self, cursor: Vec2) -> Vec2 {
        self.start.translation + self.handle.constrain(cursor - self.grab)
    }

    /// How far the drag has turned since it began, in radians.
    fn turn(&mut self, cursor: Vec2) -> f32 {
        let Some(now) = angle(cursor - self.start.translation) else {
            // The cursor is on the pivot, where no angle exists. Holding the
            // last total is the only answer that does not spin the entity by
            // whatever `atan2` returns for a zero vector.
            return self.turned;
        };
        self.turned += wrap(now - self.last_angle);
        self.last_angle = now;
        self.turned
    }

    /// The scale the entity belongs at with the cursor at `cursor`.
    ///
    /// A ratio of distances, not a sum of them: dragging a handle from 60 to
    /// 120 pixels out doubles the entity whatever it was, which is the one
    /// behaviour that composes across repeated drags. Both offsets are measured
    /// in the entity's own frame, so a turned entity grows along the arm that
    /// was grabbed rather than along the world axis it started life on.
    fn scale(&self, cursor: Vec2) -> Vec2 {
        let to_local = Mat2::from_angle(-self.start.rotation);
        let grabbed = to_local * (self.grab - self.start.translation);
        let now = to_local * (cursor - self.start.translation);

        let scaled = match self.handle {
            Handle::X => Vec2::new(
                self.start.scale.x * ratio(now.x, grabbed.x),
                self.start.scale.y,
            ),
            Handle::Y => Vec2::new(
                self.start.scale.x,
                self.start.scale.y * ratio(now.y, grabbed.y),
            ),
            // Uniform, off the distance rather than either component, so a
            // diagonal drag does not scale x and y by different amounts.
            Handle::Both => self.start.scale * ratio(now.length(), grabbed.length()),
            // The ring belongs to the rotate tool; it never reaches this.
            Handle::Ring => self.start.scale,
        };

        Vec2::new(clear_of_zero(scaled.x), clear_of_zero(scaled.y))
    }
}

/// The direction of `offset` as an angle, or `None` if it has none.
fn angle(offset: Vec2) -> Option<f32> {
    (offset.length() > MIN_OFFSET).then(|| offset.y.atan2(offset.x))
}

/// `radians` folded into `[−π, π)`.
///
/// Applied to the difference between two frames, never to the total: it is what
/// turns "the cursor jumped from +179° to −179°" into two degrees clockwise
/// instead of a 358-degree spin back the other way.
fn wrap(radians: f32) -> f32 {
    (radians + PI).rem_euclid(TAU) - PI
}

/// `now / then`, or `1.0` where `then` is too small to divide by.
fn ratio(now: f32, then: f32) -> f32 {
    if then.abs() <= MIN_OFFSET {
        return 1.0;
    }
    let ratio = now / then;
    // NaN cannot arrive from the division above, but the caller's inputs are a
    // camera projection away from arbitrary, and a NaN scale silently removes
    // the entity from the screen with no error anywhere.
    if ratio.is_finite() {
        ratio
    } else {
        1.0
    }
}

/// `value` pushed out of the dead zone around zero, keeping its sign.
fn clear_of_zero(value: f32) -> f32 {
    if value.abs() >= MIN_SCALE {
        return value;
    }
    // `is_sign_negative` rather than `< 0.0`, so −0.0 stays a flip.
    if value.is_sign_negative() {
        -MIN_SCALE
    } else {
        MIN_SCALE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};
    use voltra_ecs::World;

    /// A drag on a real entity.
    ///
    /// Spawned from a `World` because nothing public constructs an `Entity`
    /// from parts, and adding a constructor to `voltra-ecs` for a test's
    /// convenience would widen that crate's API for no other caller.
    fn drag(tool: Tool, handle: Handle, grab: Vec2, start: Transform) -> Drag {
        Drag::begin(World::new().spawn(), tool, handle, grab, start)
    }

    fn at(translation: Vec2) -> Transform {
        Transform::from_translation(translation)
    }

    #[test]
    fn a_free_drag_moves_by_the_cursor_delta() {
        let d = drag(
            Tool::Translate,
            Handle::Both,
            Vec2::new(10.0, 10.0),
            at(Vec2::new(2.0, 3.0)),
        );

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
        let d = drag(
            Tool::Translate,
            Handle::Both,
            Vec2::new(50.0, 50.0),
            at(Vec2::ZERO),
        );

        assert_eq!(d.translation(Vec2::new(51.0, 50.0)), Vec2::new(1.0, 0.0));
    }

    #[test]
    fn an_x_drag_leaves_y_exactly_alone() {
        let d = drag(
            Tool::Translate,
            Handle::X,
            Vec2::ZERO,
            at(Vec2::new(5.0, 5.0)),
        );

        assert_eq!(d.translation(Vec2::new(3.0, 99.0)), Vec2::new(8.0, 5.0));
    }

    #[test]
    fn a_y_drag_leaves_x_exactly_alone() {
        let d = drag(
            Tool::Translate,
            Handle::Y,
            Vec2::ZERO,
            at(Vec2::new(5.0, 5.0)),
        );

        assert_eq!(d.translation(Vec2::new(99.0, 3.0)), Vec2::new(5.0, 8.0));
    }

    #[test]
    fn a_drag_that_has_not_moved_changes_nothing() {
        let grab = Vec2::new(7.0, -2.0);
        let start = at(Vec2::new(1.0, 1.0));
        let mut d = drag(Tool::Translate, Handle::Both, grab, start);

        assert_eq!(d.transform(grab), start);
    }

    #[test]
    fn a_move_leaves_rotation_and_scale_alone() {
        let start = at(Vec2::ZERO)
            .with_rotation(FRAC_PI_4)
            .with_scale(Vec2::new(2.0, 3.0));
        let mut d = drag(Tool::Translate, Handle::Both, Vec2::new(5.0, 0.0), start);

        let moved = d.transform(Vec2::new(6.0, 0.0));

        assert_eq!(moved.rotation, start.rotation);
        assert_eq!(moved.scale, start.scale);
        assert_eq!(moved.translation, Vec2::new(1.0, 0.0));
    }

    #[test]
    fn a_rotate_drag_turns_by_the_angle_swept() {
        // Grabbed at +x, dragged to +y: a quarter turn counter-clockwise, which
        // is the positive direction `Transform::rotation` is documented in.
        let mut d = drag(
            Tool::Rotate,
            Handle::Ring,
            Vec2::new(4.0, 0.0),
            at(Vec2::ZERO),
        );

        let turned = d.transform(Vec2::new(0.0, 4.0));

        assert!((turned.rotation - FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn a_rotate_drag_adds_to_the_rotation_it_started_with() {
        let start = at(Vec2::ZERO).with_rotation(FRAC_PI_4);
        let mut d = drag(Tool::Rotate, Handle::Ring, Vec2::new(4.0, 0.0), start);

        let turned = d.transform(Vec2::new(0.0, 4.0));

        assert!((turned.rotation - (FRAC_PI_4 + FRAC_PI_2)).abs() < 1e-5);
    }

    #[test]
    fn a_rotate_drag_turns_about_the_entity_not_the_grab_point() {
        // The pivot is the entity's origin. Measuring the angle from the grab
        // point instead would swing the entity wildly for a small drag.
        let start = at(Vec2::new(10.0, 10.0));
        let mut d = drag(Tool::Rotate, Handle::Ring, Vec2::new(14.0, 10.0), start);

        let turned = d.transform(Vec2::new(10.0, 14.0));

        assert!((turned.rotation - FRAC_PI_2).abs() < 1e-5);
        assert_eq!(turned.translation, start.translation);
    }

    #[test]
    fn a_rotate_drag_past_half_a_turn_keeps_going() {
        // The property the running total exists for. Sampled every eighth of a
        // turn, as a dragged cursor is, three quarters of a turn must read as
        // +3π/2 and not as −π/2.
        let mut d = drag(
            Tool::Rotate,
            Handle::Ring,
            Vec2::new(4.0, 0.0),
            at(Vec2::ZERO),
        );

        let mut rotation = 0.0;
        for step in 1..=6 {
            let angle = step as f32 * FRAC_PI_4;
            let (sin, cos) = angle.sin_cos();
            rotation = d.transform(Vec2::new(cos, sin) * 4.0).rotation;
        }

        assert!(
            (rotation - 6.0 * FRAC_PI_4).abs() < 1e-4,
            "expected three quarters of a turn, got {rotation}"
        );
    }

    #[test]
    fn a_rotate_drag_the_other_way_unwinds() {
        let mut d = drag(
            Tool::Rotate,
            Handle::Ring,
            Vec2::new(4.0, 0.0),
            at(Vec2::ZERO),
        );

        d.transform(Vec2::new(0.0, 4.0));
        let back = d.transform(Vec2::new(4.0, 0.0));

        assert!(
            back.rotation.abs() < 1e-5,
            "expected 0, got {}",
            back.rotation
        );
    }

    #[test]
    fn a_rotate_drag_onto_the_pivot_holds_its_angle() {
        // No angle exists there, and `atan2(0, 0)` is 0 — which would snap the
        // entity back to its starting facing every time the cursor crossed the
        // middle of the ring.
        let mut d = drag(
            Tool::Rotate,
            Handle::Ring,
            Vec2::new(4.0, 0.0),
            at(Vec2::ZERO),
        );

        let turned = d.transform(Vec2::new(0.0, 4.0)).rotation;
        let on_pivot = d.transform(Vec2::ZERO).rotation;

        assert_eq!(on_pivot, turned);
    }

    #[test]
    fn a_rotate_leaves_translation_and_scale_alone() {
        let start = at(Vec2::new(3.0, 3.0)).with_scale(Vec2::new(2.0, 5.0));
        let mut d = drag(Tool::Rotate, Handle::Ring, Vec2::new(7.0, 3.0), start);

        let turned = d.transform(Vec2::new(3.0, 7.0));

        assert_eq!(turned.translation, start.translation);
        assert_eq!(turned.scale, start.scale);
    }

    #[test]
    fn a_scale_drag_doubles_at_twice_the_grab_distance() {
        let mut d = drag(Tool::Scale, Handle::X, Vec2::new(4.0, 0.0), at(Vec2::ZERO));

        let scaled = d.transform(Vec2::new(8.0, 0.0));

        assert!((scaled.scale.x - 2.0).abs() < 1e-5);
        assert_eq!(scaled.scale.y, 1.0);
    }

    #[test]
    fn a_scale_drag_multiplies_the_scale_it_started_with() {
        let start = at(Vec2::ZERO).with_scale(Vec2::new(3.0, 3.0));
        let mut d = drag(Tool::Scale, Handle::Y, Vec2::new(0.0, 4.0), start);

        let scaled = d.transform(Vec2::new(0.0, 2.0));

        assert!((scaled.scale.y - 1.5).abs() < 1e-5);
        assert_eq!(scaled.scale.x, 3.0);
    }

    #[test]
    fn a_uniform_scale_drag_grows_both_axes_together() {
        let mut d = drag(
            Tool::Scale,
            Handle::Both,
            Vec2::new(3.0, 4.0),
            at(Vec2::ZERO),
        );

        // Twice the distance from the origin, along a different direction: the
        // uniform handle reads distance only.
        let scaled = d.transform(Vec2::new(0.0, 10.0));

        assert!((scaled.scale.x - 2.0).abs() < 1e-5);
        assert!((scaled.scale.y - 2.0).abs() < 1e-5);
    }

    #[test]
    fn a_scale_drag_follows_the_arm_of_a_turned_entity() {
        // Turned a quarter turn, the local x arm points up the screen. Dragging
        // further up must grow local x — the axis the arm is drawn along — and
        // leave local y alone. Measuring in world axes would grow the wrong one.
        let start = at(Vec2::ZERO).with_rotation(FRAC_PI_2);
        let mut d = drag(Tool::Scale, Handle::X, Vec2::new(0.0, 4.0), start);

        let scaled = d.transform(Vec2::new(0.0, 8.0));

        assert!((scaled.scale.x - 2.0).abs() < 1e-5);
        assert!((scaled.scale.y - 1.0).abs() < 1e-5);
    }

    #[test]
    fn a_scale_drag_through_the_origin_flips_the_axis() {
        // Negative scale is a mirror, and every editor allows it. The sign has
        // to survive the dead zone around zero.
        let mut d = drag(Tool::Scale, Handle::X, Vec2::new(4.0, 0.0), at(Vec2::ZERO));

        let scaled = d.transform(Vec2::new(-4.0, 0.0));

        assert!((scaled.scale.x + 1.0).abs() < 1e-5);
    }

    #[test]
    fn a_scale_drag_never_reaches_exactly_zero() {
        let mut d = drag(
            Tool::Scale,
            Handle::Both,
            Vec2::new(4.0, 0.0),
            at(Vec2::ZERO),
        );

        let scaled = d.transform(Vec2::ZERO);

        assert!(scaled.scale.x.abs() >= MIN_SCALE, "{}", scaled.scale.x);
        assert!(scaled.scale.y.abs() >= MIN_SCALE, "{}", scaled.scale.y);
    }

    #[test]
    fn a_scale_grab_on_the_origin_leaves_the_scale_alone() {
        // Nothing to divide by. Holding the scale beats multiplying it by an
        // arbitrary number the moment the drag begins.
        let start = at(Vec2::ZERO).with_scale(Vec2::new(2.0, 2.0));
        let mut d = drag(Tool::Scale, Handle::X, Vec2::ZERO, start);

        let scaled = d.transform(Vec2::new(50.0, 0.0));

        assert_eq!(scaled.scale, start.scale);
    }

    #[test]
    fn a_scale_leaves_translation_and_rotation_alone() {
        let start = at(Vec2::new(3.0, 3.0)).with_rotation(FRAC_PI_4);
        let mut d = drag(Tool::Scale, Handle::Both, Vec2::new(7.0, 3.0), start);

        let scaled = d.transform(Vec2::new(11.0, 3.0));

        assert_eq!(scaled.translation, start.translation);
        assert_eq!(scaled.rotation, start.rotation);
    }

    #[test]
    fn wrap_folds_the_long_way_round_into_the_short_way() {
        assert!((wrap(TAU + 0.5) - 0.5).abs() < 1e-6);
        assert!((wrap(-TAU - 0.5) + 0.5).abs() < 1e-6);
        assert!(wrap(PI - 0.1) > 0.0);
        assert!(wrap(PI + 0.1) < 0.0);
    }
}
