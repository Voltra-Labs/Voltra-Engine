//! Values a client draws between two snapshots.

use voltra_render::glam::Vec2;

use crate::Transform;

/// A value that can be blended toward another: what a component
/// replicated with
/// [`replicate_interpolated`](crate::ComponentRegistry::replicate_interpolated)
/// implements, so a remote copy moves smoothly between the server's
/// snapshots instead of stepping from one to the next.
pub trait Interpolate {
    /// The value a fraction `t` (`0.0..=1.0`) of the way from `self` to
    /// `to`.
    fn interpolate(&self, to: &Self, t: f32) -> Self;
}

impl Interpolate for f32 {
    fn interpolate(&self, to: &Self, t: f32) -> Self {
        self + (to - self) * t
    }
}

impl Interpolate for Vec2 {
    /// `self + (to - self) * t`, not glam's `lerp` (`self * (1 - t) + to *
    /// t`): this form is exact when the two are equal, so an entity at rest
    /// is drawn exactly where the server has it.
    fn interpolate(&self, to: &Self, t: f32) -> Self {
        *self + (*to - *self) * t
    }
}

impl Interpolate for Transform {
    /// Position and scale linearly; the rotation the short way round, so a
    /// turn through ±π does not spin the long way.
    fn interpolate(&self, to: &Self, t: f32) -> Self {
        let turn = (to.rotation - self.rotation + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        Self {
            translation: self.translation.interpolate(&to.translation, t),
            rotation: self.rotation + turn * t,
            scale: self.scale.interpolate(&to.scale, t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rotation_takes_the_short_way() {
        let a = Transform::default().with_rotation(3.0);
        let b = Transform::default().with_rotation(-3.0);
        let half = a.interpolate(&b, 0.5);
        // Through π, not through 0.
        assert!((half.rotation.abs() - std::f32::consts::PI).abs() < 1e-4);
    }

    #[test]
    fn equal_values_interpolate_to_themselves_exactly() {
        let a = Transform::from_translation(Vec2::new(-12.621071, -3.115856)).with_rotation(15.95);
        for t in [0.125, 0.5, 0.875, 0.999] {
            assert_eq!(a.interpolate(&a, t), a);
        }
    }

    #[test]
    fn the_ends_are_the_values() {
        let a = Transform::from_translation(Vec2::new(1.0, 2.0));
        let b = Transform::from_translation(Vec2::new(3.0, -2.0)).with_scale(Vec2::splat(2.0));
        assert_eq!(a.interpolate(&b, 0.0), a);
        let end = a.interpolate(&b, 1.0);
        assert!((end.translation - b.translation).length() < 1e-6);
        assert_eq!(end.scale, b.scale);
    }
}
