//! Values a client draws between two snapshots.

use voltra_render::glam::{Quat, Vec2, Vec3};

use crate::{Transform2D, Transform3D};

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

impl Interpolate for Vec3 {
    /// `self + (to - self) * t`, exact when the two are equal (see
    /// [`Vec2`]'s).
    fn interpolate(&self, to: &Self, t: f32) -> Self {
        *self + (*to - *self) * t
    }
}

impl Interpolate for Quat {
    /// A rotation the short way round (glam's `slerp`, which turns `to`
    /// into `-to` when that is nearer: the same rotation), exactly `self`
    /// at `t <= 0` or when the two are equal and exactly `to` at `t >= 1`
    /// — glam's own is a hair off at both ends, and an entity at rest was
    /// drawn turning by an ulp. Two rotations exactly half a turn apart
    /// have no short way; the one taken is fixed by the order of the two.
    fn interpolate(&self, to: &Self, t: f32) -> Self {
        if t <= 0.0 || self == to {
            *self
        } else if t >= 1.0 {
            *to
        } else {
            self.slerp(*to, t)
        }
    }
}

impl Interpolate for Transform3D {
    /// Translation and scale linearly, the rotation the short way round
    /// ([`Quat`]'s); equal ends give exactly themselves.
    fn interpolate(&self, to: &Self, t: f32) -> Self {
        Self {
            translation: self.translation.interpolate(&to.translation, t),
            rotation: self.rotation.interpolate(&to.rotation, t),
            scale: self.scale.interpolate(&to.scale, t),
        }
    }
}

impl Interpolate for Transform2D {
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
    use voltra_testkit::rotation_apart;

    #[test]
    fn a_rotation_takes_the_short_way() {
        let a = Transform2D::default().with_rotation(3.0);
        let b = Transform2D::default().with_rotation(-3.0);
        let half = a.interpolate(&b, 0.5);
        // Through π, not through 0.
        assert!((half.rotation.abs() - std::f32::consts::PI).abs() < 1e-4);
    }

    #[test]
    fn equal_values_interpolate_to_themselves_exactly() {
        let a =
            Transform2D::from_translation(Vec2::new(-12.621071, -3.115856)).with_rotation(15.95);
        for t in [0.125, 0.5, 0.875, 0.999] {
            assert_eq!(a.interpolate(&a, t), a);
        }
    }

    #[test]
    fn the_ends_are_the_values() {
        let a = Transform2D::from_translation(Vec2::new(1.0, 2.0));
        let b = Transform2D::from_translation(Vec2::new(3.0, -2.0)).with_scale(Vec2::splat(2.0));
        assert_eq!(a.interpolate(&b, 0.0), a);
        let end = a.interpolate(&b, 1.0);
        assert!((end.translation - b.translation).length() < 1e-6);
        assert_eq!(end.scale, b.scale);
    }

    #[test]
    fn a_3d_rotation_takes_the_short_way_and_ends_exactly() {
        // 170° and -170° about Y are 20° apart through 180°, not 340°
        // through 0°.
        let a = Transform3D::default().with_rotation(Quat::from_rotation_y(170f32.to_radians()));
        let b = Transform3D::from_translation(Vec3::new(2.0, 4.0, -6.0))
            .with_rotation(Quat::from_rotation_y(-170f32.to_radians()))
            .with_scale(Vec3::splat(3.0));
        let half = a.interpolate(&b, 0.5);
        let angle = rotation_apart(half.rotation, Quat::from_rotation_y(180f32.to_radians()));
        assert!(angle < 1e-4, "through 180°, {angle} rad off");
        assert_eq!(half.translation, Vec3::new(1.0, 2.0, -3.0));
        assert_eq!(half.scale, Vec3::splat(2.0));
        let quarter = a.interpolate(&b, 0.25);
        let turned = rotation_apart(quarter.rotation, a.rotation).to_degrees();
        assert!((turned - 5.0).abs() < 1e-3, "a quarter of 20°: {turned}°");
        assert_eq!(a.interpolate(&b, 0.0), a);
        assert_eq!(a.interpolate(&b, 1.0).rotation, b.rotation, "bit for bit");
        assert_eq!(a.interpolate(&b, 1.0).translation, b.translation);
        // `-q` is `q`: the short way to it is no turn at all.
        let flipped = a.rotation.interpolate(&-a.rotation, 0.5);
        assert!(rotation_apart(flipped, a.rotation) < 1e-5);
    }

    #[test]
    fn equal_3d_values_interpolate_to_themselves_exactly() {
        let a = Transform3D::from_translation(Vec3::new(-12.621071, 3.5, -3.115856))
            .with_rotation(Quat::from_euler(
                voltra_render::glam::EulerRot::YXZ,
                0.7,
                -0.3,
                2.9,
            ))
            .with_scale(Vec3::new(0.01, 2.0, 1.0));
        for t in [0.125, 0.5, 0.875, 0.999] {
            assert_eq!(a.interpolate(&a, t), a);
        }
    }
}
