//! How far apart two 3D rotations are, measured where tests need it: near
//! zero.

use voltra_render::glam::Quat;

/// The angle between two unit rotations, in radians: `2·asin(|(a·b⁻¹).xyz|)`.
///
/// glam's `Quat::angle_between` is `2·acos(|a·b|)`, and an `acos` near 1 in
/// `f32` is off by about 10⁻³ rad — wider than most tolerances a test of
/// a rotation wants. The sine of half the turn between them is well
/// conditioned there. `q` and `−q` are 0 apart, as the rotation they are.
pub fn rotation_apart(a: Quat, b: Quat) -> f32 {
    2.0 * (a * b.conjugate()).xyz().length().min(1.0).asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_turn_is_measured_to_its_size() {
        let a = Quat::from_rotation_y(0.3);
        let b = Quat::from_rotation_y(0.3 + 1e-5);
        assert!((rotation_apart(a, b) - 1e-5).abs() < 1e-7);
        assert!(rotation_apart(a, -a) < 1e-7, "-q is q");
        let quarter = rotation_apart(Quat::IDENTITY, Quat::from_rotation_x(1.5));
        assert!((quarter - 1.5).abs() < 1e-5);
    }
}
