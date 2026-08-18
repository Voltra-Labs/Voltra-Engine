//! Which part of the gizmo a point is over.
//!
//! Everything here is in **screen pixels**, and that is the point. The gizmo is
//! drawn at a constant size on screen, so testing a click against its world
//! position would test a target that grows and shrinks with the zoom while the
//! picture does not — the exact mismatch Unreal has a standing bug report for.
//! Testing where it is drawn is the only way the two agree.
//!
//! The arms are given as directions rather than assumed to be axis-aligned:
//! translate draws its arms along the world axes (Unity's Global handle
//! orientation) while scale draws them along the selection's own, because a
//! scale is applied in local space and an arm that lies about which axis it
//! grows would be worse than no arm at all.

use voltra_render::glam::Vec2;

/// Length of each axis arm, in pixels.
pub const AXIS_LENGTH: f32 = 60.0;

/// Half-side of the centre square, in pixels.
pub const CENTRE_HALF: f32 = 8.0;

/// Radius of the rotation ring, in pixels.
///
/// The arm length, so the three gizmos claim the same piece of screen and a
/// tool change does not move the target out from under the cursor.
pub const RING_RADIUS: f32 = AXIS_LENGTH;

/// Half-side of the square drawn at a scale arm's tip, in pixels.
pub const TIP_HALF: f32 = 5.0;

/// How far from the drawn geometry still counts as a grab, in pixels.
///
/// Wider than [`LINE_WIDTH`] deliberately: a two-pixel line is a two-pixel
/// picture, not a two-pixel target, and every editor pads it.
pub const GRAB_MARGIN: f32 = 6.0;

/// Thickness of the drawn arms, in pixels.
pub const LINE_WIDTH: f32 = 2.0;

/// The part of the gizmo under the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    /// The first arm. Translate moves along world x; scale grows local x.
    X,
    /// The second arm. Translate moves along world y; scale grows local y.
    Y,
    /// The centre square. Translate moves freely; scale grows both axes.
    Both,
    /// The rotation ring. The only handle the rotate tool has.
    Ring,
}

/// The screen directions the two arms are drawn along.
///
/// Unit vectors in viewport pixels, so the y component of the world +y axis is
/// *negative*: screen y grows downward. Every sign mistake in a gizmo lives in
/// this conversion, so it is made once, here, and the rest of the module reads
/// directions off it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Axes {
    pub x: Vec2,
    pub y: Vec2,
}

impl Axes {
    /// The world axes: what the translate gizmo draws.
    pub const WORLD: Self = Self {
        x: Vec2::new(1.0, 0.0),
        y: Vec2::new(0.0, -1.0),
    };

    /// The axes of a body turned `rotation` radians counter-clockwise.
    pub fn from_rotation(rotation: f32) -> Self {
        let (sin, cos) = rotation.sin_cos();
        Self {
            x: Vec2::new(cos, -sin),
            y: Vec2::new(-sin, -cos),
        }
    }
}

impl Handle {
    /// The handle of an arm gizmo at `cursor`, both points in viewport pixels.
    ///
    /// The centre square is tested first because both arms begin inside it;
    /// tested last it could never be hit, which would make the smallest target
    /// in the gizmo the unreachable one.
    pub fn on_arms(cursor: Vec2, origin: Vec2, axes: Axes) -> Option<Self> {
        let d = cursor - origin;

        if d.x.abs() <= CENTRE_HALF && d.y.abs() <= CENTRE_HALF {
            return Some(Self::Both);
        }

        for (handle, direction) in [(Self::X, axes.x), (Self::Y, axes.y)] {
            let tip = origin + direction * AXIS_LENGTH;
            if distance_to_segment(cursor, origin, tip) <= GRAB_MARGIN {
                return Some(handle);
            }
        }

        None
    }

    /// The handle of the rotation gizmo at `cursor`, in viewport pixels.
    ///
    /// An annulus, not a disc: the inside of the ring stays free so a click
    /// there still selects whatever sprite is under it. Unity, Godot and
    /// Blender all leave the middle of a rotation ring clickable.
    pub fn on_ring(cursor: Vec2, origin: Vec2) -> Option<Self> {
        let off_ring = (cursor.distance(origin) - RING_RADIUS).abs();
        (off_ring <= GRAB_MARGIN).then_some(Self::Ring)
    }

    /// `delta` with the axes this handle does not move zeroed out.
    pub fn constrain(self, delta: Vec2) -> Vec2 {
        match self {
            Self::X => Vec2::new(delta.x, 0.0),
            Self::Y => Vec2::new(0.0, delta.y),
            Self::Both | Self::Ring => delta,
        }
    }
}

/// Distance from `p` to the segment `a`–`b`, in the units they are given in.
///
/// Clamped to the segment rather than the infinite line: an arm is 60 pixels
/// long, and the line it lies on crosses the whole viewport.
fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let span = b - a;
    let length_squared = span.length_squared();
    if length_squared <= f32::EPSILON {
        // A degenerate arm — a local axis scaled to nothing. Fall back to the
        // endpoint rather than dividing by zero.
        return p.distance(a);
    }
    let t = ((p - a).dot(span) / length_squared).clamp(0.0, 1.0);
    p.distance(a + span * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    /// The gizmo's origin in these tests, in screen pixels.
    const O: Vec2 = Vec2::new(100.0, 100.0);

    #[test]
    fn the_centre_square_is_hit_at_the_origin() {
        assert_eq!(Handle::on_arms(O, O, Axes::WORLD), Some(Handle::Both));
    }

    #[test]
    fn a_point_along_the_x_arm_hits_x() {
        let on_x = O + Vec2::new(AXIS_LENGTH * 0.75, 0.0);
        assert_eq!(Handle::on_arms(on_x, O, Axes::WORLD), Some(Handle::X));
    }

    #[test]
    fn a_point_along_the_y_arm_hits_y() {
        // Screen y grows downward, so the Y arm runs to negative y.
        let on_y = O + Vec2::new(0.0, -AXIS_LENGTH * 0.75);
        assert_eq!(Handle::on_arms(on_y, O, Axes::WORLD), Some(Handle::Y));
    }

    #[test]
    fn the_arm_below_the_origin_is_not_the_y_handle() {
        // The Y arm points up only. Bidirectional arms would make the quadrant
        // below-left grabbable with nothing drawn there.
        let below = O + Vec2::new(0.0, AXIS_LENGTH * 0.75);
        assert_eq!(Handle::on_arms(below, O, Axes::WORLD), None);
    }

    #[test]
    fn just_outside_the_grab_margin_misses() {
        let near = O + Vec2::new(AXIS_LENGTH * 0.5, GRAB_MARGIN + 1.0);
        assert_eq!(Handle::on_arms(near, O, Axes::WORLD), None);
    }

    #[test]
    fn just_inside_the_grab_margin_hits() {
        // The margin is wider than the drawn line on purpose.
        let near = O + Vec2::new(AXIS_LENGTH * 0.5, GRAB_MARGIN - 1.0);
        assert_eq!(Handle::on_arms(near, O, Axes::WORLD), Some(Handle::X));
    }

    #[test]
    fn the_centre_wins_where_it_overlaps_an_arm() {
        let inside_square = O + Vec2::new(CENTRE_HALF * 0.5, 0.0);
        assert_eq!(
            Handle::on_arms(inside_square, O, Axes::WORLD),
            Some(Handle::Both)
        );
    }

    #[test]
    fn a_point_beyond_the_arm_misses() {
        let past = O + Vec2::new(AXIS_LENGTH + GRAB_MARGIN + 1.0, 0.0);
        assert_eq!(Handle::on_arms(past, O, Axes::WORLD), None);
    }

    #[test]
    fn a_turned_gizmo_moves_its_arms_with_it() {
        // A quarter turn counter-clockwise puts the local x arm where the y arm
        // of an unturned gizmo was. Hit testing the drawn position rather than
        // the world axes is the whole reason `Axes` is a parameter.
        let axes = Axes::from_rotation(FRAC_PI_2);
        let up = O + Vec2::new(0.0, -AXIS_LENGTH * 0.75);

        assert_eq!(Handle::on_arms(up, O, axes), Some(Handle::X));
        assert_eq!(Handle::on_arms(up, O, Axes::WORLD), Some(Handle::Y));
    }

    #[test]
    fn a_turned_gizmo_leaves_its_old_arm_empty() {
        let axes = Axes::from_rotation(FRAC_PI_2);
        let right = O + Vec2::new(AXIS_LENGTH * 0.75, 0.0);

        // The x arm has turned away; nothing is drawn to the right any more.
        assert_eq!(Handle::on_arms(right, O, axes), None);
    }

    #[test]
    fn unrotated_axes_are_the_world_axes() {
        let axes = Axes::from_rotation(0.0);
        assert!((axes.x - Axes::WORLD.x).length() < 1e-6);
        assert!((axes.y - Axes::WORLD.y).length() < 1e-6);
    }

    #[test]
    fn the_ring_is_hit_at_its_radius() {
        let on_ring = O + Vec2::new(RING_RADIUS, 0.0);
        assert_eq!(Handle::on_ring(on_ring, O), Some(Handle::Ring));
    }

    #[test]
    fn the_ring_is_hit_from_every_direction() {
        for turn in 0..8 {
            let angle = turn as f32 * std::f32::consts::FRAC_PI_4;
            let (sin, cos) = angle.sin_cos();
            let on_ring = O + Vec2::new(cos, sin) * RING_RADIUS;
            assert_eq!(
                Handle::on_ring(on_ring, O),
                Some(Handle::Ring),
                "at {angle}"
            );
        }
    }

    #[test]
    fn the_middle_of_the_ring_is_not_a_handle() {
        // Left clickable on purpose: a ring that swallows its own middle would
        // make the selected sprite unselectable-through and every click on it a
        // rotation.
        assert_eq!(Handle::on_ring(O, O), None);
    }

    #[test]
    fn outside_the_ring_is_not_a_handle() {
        let past = O + Vec2::new(RING_RADIUS + GRAB_MARGIN + 1.0, 0.0);
        assert_eq!(Handle::on_ring(past, O), None);
    }

    #[test]
    fn x_constrains_a_delta_to_its_axis() {
        assert_eq!(
            Handle::X.constrain(Vec2::new(3.0, 7.0)),
            Vec2::new(3.0, 0.0)
        );
    }

    #[test]
    fn y_constrains_a_delta_to_its_axis() {
        assert_eq!(
            Handle::Y.constrain(Vec2::new(3.0, 7.0)),
            Vec2::new(0.0, 7.0)
        );
    }

    #[test]
    fn both_passes_a_delta_through_untouched() {
        let delta = Vec2::new(3.0, 7.0);
        assert_eq!(Handle::Both.constrain(delta), delta);
    }

    #[test]
    fn the_distance_to_a_degenerate_segment_is_the_distance_to_its_point() {
        let p = Vec2::new(3.0, 4.0);
        assert!((distance_to_segment(p, Vec2::ZERO, Vec2::ZERO) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn the_distance_to_a_segment_is_clamped_to_its_ends() {
        // Beyond the tip, the nearest point is the tip — not the foot of the
        // perpendicular on the infinite line, which would make an arm grabbable
        // for the whole width of the viewport.
        let a = Vec2::ZERO;
        let b = Vec2::new(10.0, 0.0);
        assert!((distance_to_segment(Vec2::new(20.0, 0.0), a, b) - 10.0).abs() < 1e-6);
    }
}
