//! Which way a placed mesh's front faces wind on screen.

use glam::{Affine3A, Mat3};

/// Which way a placed mesh's front faces wind on screen:
/// [`Winding::of`]. Every 3D pipeline has a variant per winding,
/// and the draw list keeps instances of each apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Winding {
    /// As authored: counter-clockwise fronts, back faces culled.
    #[default]
    CounterClockwise,
    /// Mirrored, a negative determinant: clockwise fronts, back faces
    /// culled.
    Clockwise,
    /// Flattened to a plane: neither way can be trusted, so no face is
    /// culled.
    Flat,
}

impl Winding {
    /// Every winding, in key order.
    pub const ALL: [Self; 3] = [Self::CounterClockwise, Self::Clockwise, Self::Flat];

    /// Which way `model` turns a mesh's triangles on screen, and so which
    /// pipeline draws it
    /// ([`SurfaceKey::winding`](super::SurfaceKey::winding)),
    /// by the sign of the determinant, unless that is rounding noise.
    ///
    /// A negative determinant (an odd number of negative scales) mirrors
    /// space and turns the fronts clockwise, as Unity and Godot flip the
    /// front face for a negatively scaled renderer. A model flattened to a
    /// plane has a determinant of zero, which a turn before and after the
    /// flattening leaves as noise of either sign (some 10⁻⁹): judged by its
    /// sign, a plane would show and vanish between neighbouring angles. So a
    /// determinant under [`FLAT_TOLERANCE`](Self::FLAT_TOLERANCE) times
    /// ‖cofactor‖·‖M‖ — both Frobenius norms, which makes the test blind to
    /// scale, a uniform 10⁻⁴ model being as solid as a unit one — is
    /// [`Flat`](Self::Flat), drawn with no face culled.
    pub fn of(model: &Affine3A) -> Winding {
        let m = Mat3::from(model.matrix3);
        let (a, b, c) = (m.x_axis, m.y_axis, m.z_axis);
        let cofactor = Mat3::from_cols(b.cross(c), c.cross(a), a.cross(b));
        let det = a.dot(cofactor.x_axis);
        let norm = |m: Mat3| {
            (m.x_axis.length_squared() + m.y_axis.length_squared() + m.z_axis.length_squared())
                .sqrt()
        };
        // Not finite (a broken model) counts as flat: no face is trusted.
        let bound = Self::FLAT_TOLERANCE * norm(cofactor) * norm(m);
        if !det.is_finite() || !bound.is_finite() || det.abs() <= bound {
            Self::Flat
        } else if det < 0.0 {
            Self::Clockwise
        } else {
            Self::CounterClockwise
        }
    }

    /// How small a determinant is, against ‖cofactor‖·‖M‖, before a model
    /// counts as [flat](Winding::Flat): about a hundred `f32` roundings.
    pub const FLAT_TOLERANCE: f32 = 1.0e-5;
}

#[cfg(test)]
mod tests {
    use glam::{Quat, Vec3};

    use super::*;
    use crate::render3d::Instance3D;

    #[test]
    fn a_model_flattened_along_a_turned_axis_is_flat_at_every_angle() {
        // Turned, flattened, turned again: the determinant is rounding noise
        // of either sign, which once flipped the winding between angles.
        let flatten = Affine3A::from_scale(Vec3::new(1.0, 1.0, 0.0));
        let mut flat = 0;
        for step in 0..720 {
            let angle = step as f32 * 0.5f32.to_radians();
            let outer = Affine3A::from_quat(Quat::from_euler(
                glam::EulerRot::YXZ,
                angle,
                0.7 * angle,
                0.3,
            ));
            let inner = Affine3A::from_quat(Quat::from_rotation_x(1.1 * angle + 0.2));
            let model = outer * flatten * inner;
            assert_eq!(Winding::of(&model), Winding::Flat, "at {angle}");
            assert!(Instance3D::new(model, [1.0; 4]).is_ok(), "drawn at {angle}");
            flat += 1;
            // Mirrored and flattened, the open plane at (−1, 1, 0): flat too.
            let mirrored = outer * Affine3A::from_scale(Vec3::new(-1.0, 1.0, 0.0)) * inner;
            assert_eq!(Winding::of(&mirrored), Winding::Flat, "at {angle}");
        }
        assert_eq!(flat, 720);
        // Thin but solid, at any size, keeps its sign.
        for scale in [Vec3::new(1.0, 1.0, 1.0e-3), Vec3::splat(1.0e-4)] {
            let thin = Affine3A::from_scale(scale);
            assert_eq!(Winding::of(&thin), Winding::CounterClockwise);
            assert_eq!(
                Winding::of(&Affine3A::from_scale(scale * Vec3::new(-1.0, 1.0, 1.0))),
                Winding::Clockwise
            );
        }
    }
}
