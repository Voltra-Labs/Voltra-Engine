//! A 3D view: where the eye is, and how it projects — reversed Z.

use glam::camera::rh::proj::directx;
use glam::{Affine3A, Mat4, Vec3};

use super::frustum::Frustum;

/// The view and projection a 3D camera draws through.
///
/// Right-handed, Y up, looking down the placement's −Z (glTF's, Godot's and
/// Bevy's convention). The projection is wgpu's clip space — Z in `[0, 1]`
/// — **reversed**: the near plane is depth 1 and the far plane (or
/// infinity) depth 0, which with a `Depth32Float` target cleared to 0 and
/// compared `GreaterEqual` spreads a float's precision evenly over
/// distance. On the GL backend the precision gain is lost but the
/// comparison still holds (ARCHITECTURE, "The 3D pass").
///
/// The constructors take what a camera component holds, already in
/// radians and metres, and never produce a non-finite matrix: a near plane
/// that is not positive becomes [`DEFAULT_NEAR`](Self::DEFAULT_NEAR), both
/// planes are kept within 10¹⁸ m, a far plane not past the near one is
/// dropped (perspective: infinite) or moved past it (orthographic: a metre
/// or a thousandth, whichever is more), the field of view is kept inside
/// `(0°, 180°)` and the aspect positive. The scene side warns about such a camera; this side
/// only refuses to draw garbage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View3D {
    /// World space to view space: the inverse of the camera's placement
    /// with its scale left out.
    pub view: Mat4,
    /// View space to clip space, reversed Z.
    pub projection: Mat4,
    /// Where the eye is, in the world.
    pub eye: Vec3,
    /// The way it looks, unit length, in the world.
    pub forward: Vec3,
}

impl View3D {
    /// The near plane a camera has unless told otherwise: 0.1 m (Bevy's;
    /// Unity uses 0.3, Godot 0.05).
    pub const DEFAULT_NEAR: f32 = 0.1;

    /// A perspective view from `placement`, `fov_y` radians high, `aspect`
    /// wide over high, from `near` to `far` — infinite for `None`.
    pub fn perspective(
        placement: Affine3A,
        fov_y: f32,
        aspect: f32,
        near: f32,
        far: Option<f32>,
    ) -> Self {
        let near = sane_near(near);
        let fov_y = if fov_y.is_finite() {
            fov_y.clamp(1.0e-3, std::f32::consts::PI - 1.0e-3)
        } else {
            std::f32::consts::FRAC_PI_3
        };
        let aspect = sane_aspect(aspect);
        let far = far
            .filter(|far| far.is_finite())
            .map(|far| far.min(MAX_PLANE))
            .filter(|far| *far > near);
        let projection = match far {
            // Reversed by handing the planes over swapped: near maps to 1.
            Some(far) => directx::perspective(fov_y, aspect, far, near),
            None => directx::perspective_infinite_reverse(fov_y, aspect, near),
        };
        Self::placed(placement, projection)
    }

    /// An orthographic view from `placement`, `half_height` metres above
    /// and below its centre, `aspect` wide over high, from `near` to `far`.
    pub fn orthographic(
        placement: Affine3A,
        half_height: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> Self {
        let near = if near.is_finite() {
            near.clamp(0.0, MAX_PLANE)
        } else {
            0.0
        };
        // Past `near` by a metre or a thousandth of it, whichever is more:
        // `near + 1` is `near` again from 1e8 up.
        let far = if far.is_finite() && far > near {
            far
        } else {
            (near + 1.0).max(near * 1.001)
        };
        let half_height = if half_height.is_finite() && half_height > 0.0 {
            half_height
        } else {
            1.0
        };
        let half_width = half_height * sane_aspect(aspect);
        let projection = directx::orthographic(
            -half_width,
            half_width,
            -half_height,
            half_height,
            far,
            near,
        );
        Self::placed(placement, projection)
    }

    fn placed(placement: Affine3A, projection: Mat4) -> Self {
        let (_, rotation, eye) = placement.to_scale_rotation_translation();
        // A collapsed placement (zero scale) has no rotation to read.
        let rotation = if rotation.is_finite() && rotation.length() > 0.5 {
            rotation.normalize()
        } else {
            glam::Quat::IDENTITY
        };
        let eye = if eye.is_finite() { eye } else { Vec3::ZERO };
        let rigid = Mat4::from_rotation_translation(rotation, eye);
        Self {
            view: rigid.inverse(),
            projection,
            eye,
            forward: rotation * Vec3::NEG_Z,
        }
    }

    /// World space to clip space.
    pub fn view_projection(&self) -> Mat4 {
        self.projection * self.view
    }

    /// Whether it projects orthographically: every ray parallel, the eye
    /// at no point. A perspective matrix leaves `w` for depth (its last
    /// entry 0); an orthographic one keeps it 1.
    pub fn is_orthographic(&self) -> bool {
        self.projection.w_axis.w != 0.0
    }

    /// The planes around what it sees.
    pub fn frustum(&self) -> Frustum {
        Frustum::from_view_projection(self.view_projection())
    }

    /// How far in front of the eye `point` is, along [`forward`](Self::forward):
    /// what draws are sorted by.
    pub fn depth(&self, point: Vec3) -> f32 {
        (point - self.eye).dot(self.forward)
    }
}

/// The furthest a near plane may be: far enough for any scene, near enough
/// that a far plane a thousandth past it, and the matrices made from both
/// (a perspective one multiplies the two), stay finite.
const MAX_PLANE: f32 = 1.0e18;

fn sane_near(near: f32) -> f32 {
    if near.is_finite() && near > 0.0 {
        near.min(MAX_PLANE)
    } else {
        View3D::DEFAULT_NEAR
    }
}

fn sane_aspect(aspect: f32) -> f32 {
    if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use glam::{Quat, Vec4};

    use super::*;

    fn depth_at(view: &View3D, distance: f32) -> f32 {
        let clip = view.view_projection() * Vec4::new(0.0, 0.0, -distance, 1.0);
        clip.z / clip.w
    }

    #[test]
    fn reversed_z_puts_near_at_one_and_far_at_zero() {
        let finite = View3D::perspective(Affine3A::IDENTITY, 1.0, 1.5, 0.1, Some(100.0));
        assert!((depth_at(&finite, 0.1) - 1.0).abs() < 1e-6);
        assert!(depth_at(&finite, 100.0).abs() < 1e-6);
        assert!(
            depth_at(&finite, 1.0) > depth_at(&finite, 2.0),
            "nearer is greater"
        );

        let infinite = View3D::perspective(Affine3A::IDENTITY, 1.0, 1.5, 0.1, None);
        assert!((depth_at(&infinite, 0.1) - 1.0).abs() < 1e-6);
        assert!(depth_at(&infinite, 1.0e7) < 1e-7);

        let ortho = View3D::orthographic(Affine3A::IDENTITY, 5.0, 1.0, 0.1, 50.0);
        assert!((depth_at(&ortho, 0.1) - 1.0).abs() < 1e-6);
        assert!(depth_at(&ortho, 50.0).abs() < 1e-6);
    }

    #[test]
    fn the_view_looks_down_the_placements_minus_z_and_ignores_scale() {
        let placement = Affine3A::from_scale_rotation_translation(
            Vec3::splat(3.0),
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Vec3::new(1.0, 2.0, 3.0),
        );
        let view = View3D::perspective(placement, 1.0, 1.0, 0.1, None);
        // Turned a quarter left about Y, −Z becomes −X.
        assert!((view.forward - Vec3::NEG_X).length() < 1e-6);
        assert!((view.eye - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-6);
        let ahead = view.view.transform_point3(Vec3::new(-4.0, 2.0, 3.0));
        assert!(
            (ahead - Vec3::new(0.0, 0.0, -5.0)).length() < 1e-5,
            "{ahead}"
        );
        assert!((view.depth(Vec3::new(-4.0, 2.0, 3.0)) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn broken_numbers_never_make_a_broken_matrix() {
        let broken = Affine3A::from_translation(Vec3::splat(f32::NAN));
        for view in [
            View3D::perspective(broken, f32::NAN, 0.0, -1.0, Some(f32::NAN)),
            View3D::perspective(Affine3A::IDENTITY, 0.0, f32::INFINITY, 0.0, Some(0.0)),
            View3D::orthographic(broken, 0.0, -2.0, f32::NAN, -5.0),
            View3D::orthographic(Affine3A::IDENTITY, 1.0, 1.0, 1.0e8, 0.0),
            View3D::orthographic(Affine3A::IDENTITY, 1.0, 1.0, 1.0e38, f32::MAX),
            View3D::perspective(Affine3A::IDENTITY, 1.0, 1.0, 1.0e38, None),
            View3D::perspective(Affine3A::IDENTITY, 1.0, 1.0, f32::MAX, Some(f32::MAX)),
        ] {
            assert!(view.view_projection().is_finite(), "{view:?}");
        }
    }
}
