//! Which part of the gizmo a point is over.
//!
//! Everything here is in **screen pixels**, and that is the point. The gizmo is
//! drawn at a constant size on screen, so testing a click against its world
//! position would test a target that grows and shrinks with the zoom while the
//! picture does not — the exact mismatch Unreal has a standing bug report for.
//! Testing where it is drawn is the only way the two agree.

use voltra_render::glam::Vec2;

/// Length of each axis arm, in pixels.
pub const AXIS_LENGTH: f32 = 60.0;

/// Half-side of the centre square, in pixels.
pub const CENTRE_HALF: f32 = 8.0;

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
    /// The X arm. A drag moves along world x only.
    X,
    /// The Y arm. A drag moves along world y only.
    Y,
    /// The centre square. A drag moves freely.
    Both,
}

impl Handle {
    /// The handle at `cursor`, both points in viewport-local pixels.
    ///
    /// The centre square is tested first because both arms begin inside it;
    /// tested last it could never be hit, which would make the smallest target
    /// in the gizmo the unreachable one.
    pub fn at(cursor: Vec2, origin: Vec2) -> Option<Self> {
        let d = cursor - origin;

        if d.x.abs() <= CENTRE_HALF && d.y.abs() <= CENTRE_HALF {
            return Some(Self::Both);
        }

        // Screen y grows downward, so the arm drawn *upward* is negative y.
        // Getting this backwards puts the Y handle where nobody points at it.
        if (0.0..=AXIS_LENGTH + GRAB_MARGIN).contains(&d.x) && d.y.abs() <= GRAB_MARGIN {
            return Some(Self::X);
        }
        if (-(AXIS_LENGTH + GRAB_MARGIN)..=0.0).contains(&d.y) && d.x.abs() <= GRAB_MARGIN {
            return Some(Self::Y);
        }

        None
    }

    /// `delta` with the axes this handle does not move zeroed out.
    pub fn constrain(self, delta: Vec2) -> Vec2 {
        match self {
            Self::X => Vec2::new(delta.x, 0.0),
            Self::Y => Vec2::new(0.0, delta.y),
            Self::Both => delta,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gizmo's origin in these tests, in screen pixels.
    const O: Vec2 = Vec2::new(100.0, 100.0);

    #[test]
    fn the_centre_square_is_hit_at_the_origin() {
        assert_eq!(Handle::at(O, O), Some(Handle::Both));
    }

    #[test]
    fn a_point_along_the_x_arm_hits_x() {
        let on_x = O + Vec2::new(AXIS_LENGTH * 0.75, 0.0);
        assert_eq!(Handle::at(on_x, O), Some(Handle::X));
    }

    #[test]
    fn a_point_along_the_y_arm_hits_y() {
        // Screen y grows downward, so the Y arm runs to negative y.
        let on_y = O + Vec2::new(0.0, -AXIS_LENGTH * 0.75);
        assert_eq!(Handle::at(on_y, O), Some(Handle::Y));
    }

    #[test]
    fn the_arm_below_the_origin_is_not_the_y_handle() {
        // The Y arm points up only. Bidirectional arms would make the quadrant
        // below-left grabbable with nothing drawn there.
        let below = O + Vec2::new(0.0, AXIS_LENGTH * 0.75);
        assert_eq!(Handle::at(below, O), None);
    }

    #[test]
    fn just_outside_the_grab_margin_misses() {
        let near = O + Vec2::new(AXIS_LENGTH * 0.5, GRAB_MARGIN + 1.0);
        assert_eq!(Handle::at(near, O), None);
    }

    #[test]
    fn just_inside_the_grab_margin_hits() {
        // The margin is wider than the drawn line on purpose.
        let near = O + Vec2::new(AXIS_LENGTH * 0.5, GRAB_MARGIN - 1.0);
        assert_eq!(Handle::at(near, O), Some(Handle::X));
    }

    #[test]
    fn the_centre_wins_where_it_overlaps_an_arm() {
        let inside_square = O + Vec2::new(CENTRE_HALF * 0.5, 0.0);
        assert_eq!(Handle::at(inside_square, O), Some(Handle::Both));
    }

    #[test]
    fn a_point_beyond_the_arm_misses() {
        let past = O + Vec2::new(AXIS_LENGTH + GRAB_MARGIN + 1.0, 0.0);
        assert_eq!(Handle::at(past, O), None);
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
}
