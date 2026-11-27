//! Constant speed on slopes: a motion across turned to run along a floor,
//! covering the same ground across as it would on the flat.
//!
//! Without it, the part of a walk that goes into a slope is taken off, so a
//! slope walked up is slower (by its cosine squared, for a walk straight
//! across), and the velocity kept rises with the slope and carries on up
//! past its top; walked down, the character steps off into the air and the
//! snap brings it back. With it, the velocity kept is the walk across. Godot's
//! `floor_constant_speed` keeps the speed along the slope; this keeps it
//! across `up`, so where the character is across the level does not depend
//! on the ground it crossed — what a platformer's level is laid out by.

use voltra_render::glam::Vec2;

/// A floor whose slope runs nearer than this to straight along `up`
/// (`cos` of the angle between the motion across and the floor) is too
/// steep to stretch a motion along: it would be stretched without bound.
const STEEPEST: f32 = 1e-3;

/// `motion`'s part across `up`, run along the floor of `normal` instead:
/// the same distance across, and up or down the slope as far as that
/// takes it. Nothing across — standing, or falling straight — is nothing.
pub(super) fn along_floor(motion: Vec2, normal: Vec2, up: Vec2) -> Vec2 {
    let across = motion - up * motion.dot(up);
    let length = across.length();
    if length <= 0.0 || !length.is_finite() {
        return Vec2::ZERO;
    }
    let mut slope = normal.perp();
    if slope.dot(across) < 0.0 {
        slope = -slope;
    }
    let cosine = slope.dot(across / length);
    if cosine < STEEPEST {
        return across;
    }
    slope * (length / cosine)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_walk_runs_up_and_down_a_slope_as_far_across() {
        let normal = Vec2::from_angle(30.0f32.to_radians()).perp();
        // Up the slope, rising to the right.
        let up = along_floor(Vec2::new(1.0, -0.2), normal, Vec2::Y);
        assert!((up.x - 1.0).abs() < 1e-5, "{up}");
        assert!((up.y - 30.0f32.to_radians().tan()).abs() < 1e-5, "{up}");
        // And down it, to the left.
        let down = along_floor(Vec2::new(-1.0, 0.0), normal, Vec2::Y);
        assert!((down.x + 1.0).abs() < 1e-5, "{down}");
        assert!((down.y + 30.0f32.to_radians().tan()).abs() < 1e-5, "{down}");
        // Flat ground leaves it across; falling straight is nothing.
        let flat = along_floor(Vec2::new(2.0, -1.0), Vec2::Y, Vec2::Y);
        assert!(flat.distance(Vec2::new(2.0, 0.0)) < 1e-6, "{flat}");
        assert_eq!(along_floor(Vec2::NEG_Y, normal, Vec2::Y), Vec2::ZERO);
    }
}
