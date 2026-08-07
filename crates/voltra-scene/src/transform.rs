//! Where a thing is, how big it is, and which way it faces.

use voltra_render::glam::{Mat3, Vec2};

/// A 2D position, rotation and scale.
///
/// `Mat3` rather than `Mat4`: a 2D affine transform needs nine floats, not
/// sixteen, and the camera promotes it on the way to the shader.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Transform {
    pub translation: Vec2,
    /// Counter-clockwise, in radians.
    pub rotation: f32,
    pub scale: Vec2,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec2::ZERO,
            rotation: 0.0,
            // Not `Vec2::ZERO`: a default-constructed entity should be
            // visible, not collapsed to a point.
            scale: Vec2::ONE,
        }
    }
}

impl Transform {
    pub fn from_translation(translation: Vec2) -> Self {
        Self {
            translation,
            ..Default::default()
        }
    }

    pub fn with_scale(mut self, scale: Vec2) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_rotation(mut self, radians: f32) -> Self {
        self.rotation = radians;
        self
    }

    /// Local space to world space.
    ///
    /// Scale, then rotate, then translate. Any other order makes an object's
    /// position depend on its size or facing.
    pub fn matrix(&self) -> Mat3 {
        Mat3::from_scale_angle_translation(self.scale, self.rotation, self.translation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    fn apply(transform: &Transform, point: Vec2) -> Vec2 {
        transform.matrix().transform_point2(point)
    }

    #[test]
    fn default_transform_leaves_a_point_alone() {
        let point = Vec2::new(3.0, -4.0);
        assert!((apply(&Transform::default(), point) - point).length() < 1e-6);
    }

    #[test]
    fn translation_moves_the_point() {
        let t = Transform::from_translation(Vec2::new(2.0, 1.0));
        assert!((apply(&t, Vec2::ZERO) - Vec2::new(2.0, 1.0)).length() < 1e-6);
    }

    #[test]
    fn scale_multiplies_around_the_origin() {
        let t = Transform::default().with_scale(Vec2::new(2.0, 3.0));
        assert!((apply(&t, Vec2::new(1.0, 1.0)) - Vec2::new(2.0, 3.0)).length() < 1e-6);
    }

    #[test]
    fn rotation_is_counter_clockwise_in_radians() {
        let t = Transform::default().with_rotation(FRAC_PI_2);
        // +X rotated a quarter turn counter-clockwise lands on +Y.
        assert!((apply(&t, Vec2::X) - Vec2::Y).length() < 1e-6);
    }

    #[test]
    fn scale_is_applied_before_translation() {
        let t = Transform::from_translation(Vec2::new(10.0, 0.0)).with_scale(Vec2::splat(2.0));

        // Scale-then-translate puts this at 10 + 2; the other order would put
        // it at (1 + 10) * 2 = 22. Getting this backwards makes objects drift
        // further from the origin the bigger they are.
        assert!((apply(&t, Vec2::new(1.0, 0.0)) - Vec2::new(12.0, 0.0)).length() < 1e-6);
    }

    #[test]
    fn with_scale_builds_a_uniform_square() {
        let t = Transform::from_translation(Vec2::ZERO).with_scale(Vec2::splat(4.0));
        assert_eq!(t.scale, Vec2::splat(4.0));
    }
}
