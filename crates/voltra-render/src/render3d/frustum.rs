//! The part of the world a 3D view sees, as planes to cull against.

use glam::{Mat4, Vec3, Vec4};

use super::bounds::Aabb3;

/// The planes around what a view sees: left, right, bottom, top, near and,
/// for a finite projection, far.
///
/// Each plane is `(normal, d)` with the normal pointing in, so a point is
/// inside when `normal · p + d >= 0` for every plane. Extracted from the
/// view-projection matrix (Gribb and Hartmann) for wgpu's clip space, `0 <=
/// z <= w` — written for reversed Z, where `z = w` is the near plane and
/// `z = 0` the far one. An infinite projection's far row is degenerate
/// (no normal, `d` the near distance): it bounds nothing and is left out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frustum {
    planes: [Vec4; 6],
    count: usize,
}

impl Frustum {
    /// The frustum of `view_projection`, world space to clip space.
    pub fn from_view_projection(view_projection: Mat4) -> Self {
        let [r0, r1, r2, r3] = [0, 1, 2, 3].map(|i| view_projection.row(i));
        let mut planes = [Vec4::ZERO; 6];
        let mut count = 0;
        for plane in [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r3 - r2, r2] {
            let length = plane.truncate().length();
            if length > f32::EPSILON && length.is_finite() {
                planes[count] = plane / length;
                count += 1;
            }
        }
        Self { planes, count }
    }

    /// The planes, inward normals, normalised.
    pub fn planes(&self) -> &[Vec4] {
        &self.planes[..self.count]
    }

    /// Whether any of `bounds` may be inside. Conservative: a box near a
    /// corner of the frustum can answer `true` while wholly outside, never
    /// `false` while partly inside. A non-finite box answers `false`.
    pub fn intersects(&self, bounds: &Aabb3) -> bool {
        if !bounds.is_finite() {
            return false;
        }
        self.planes().iter().all(|plane| {
            let normal = plane.truncate();
            // The corner furthest along the normal: if even it is behind the
            // plane, the whole box is.
            let corner = Vec3::select(normal.cmpge(Vec3::ZERO), bounds.max, bounds.min);
            normal.dot(corner) + plane.w >= 0.0
        })
    }

    /// Whether `point` is inside.
    pub fn contains(&self, point: Vec3) -> bool {
        self.planes()
            .iter()
            .all(|plane| plane.truncate().dot(point) + plane.w >= 0.0)
    }
}

#[cfg(test)]
mod tests {
    use glam::Affine3A;

    use super::*;
    use crate::render3d::View3D;

    fn cube(at: Vec3) -> Aabb3 {
        Aabb3::new(at - Vec3::splat(0.5), at + Vec3::splat(0.5))
    }

    /// At the origin looking down −Z, 90° high, square, near 0.1.
    fn perspective(far: Option<f32>) -> Frustum {
        View3D::perspective(
            Affine3A::IDENTITY,
            std::f32::consts::FRAC_PI_2,
            1.0,
            0.1,
            far,
        )
        .frustum()
    }

    #[test]
    fn an_infinite_perspective_has_five_planes_and_sees_far() {
        let frustum = perspective(None);
        assert_eq!(frustum.planes().len(), 5);
        assert!(frustum.intersects(&cube(Vec3::new(0.0, 0.0, -5.0))));
        assert!(frustum.intersects(&cube(Vec3::new(0.0, 0.0, -1.0e6))));
    }

    #[test]
    fn a_box_past_each_plane_is_culled() {
        let frustum = perspective(Some(100.0));
        assert_eq!(frustum.planes().len(), 6);
        // 90°: at 10 m in front the view spans ±10 m, so ±12 is outside.
        for (outside, plane) in [
            (Vec3::new(-12.0, 0.0, -10.0), "left"),
            (Vec3::new(12.0, 0.0, -10.0), "right"),
            (Vec3::new(0.0, -12.0, -10.0), "bottom"),
            (Vec3::new(0.0, 12.0, -10.0), "top"),
            (Vec3::new(0.0, 0.0, 2.0), "near (behind the camera)"),
            (Vec3::new(0.0, 0.0, -101.0), "far"),
        ] {
            assert!(
                !frustum.intersects(&cube(outside)),
                "past the {plane} plane"
            );
        }
        // Straddling each plane is kept.
        for straddling in [
            Vec3::new(-10.2, 0.0, -10.0),
            Vec3::new(10.2, 0.0, -10.0),
            Vec3::new(0.0, -10.2, -10.0),
            Vec3::new(0.0, 10.2, -10.0),
            Vec3::new(0.0, 0.0, 0.3),
            Vec3::new(0.0, 0.0, -100.3),
        ] {
            assert!(frustum.intersects(&cube(straddling)), "{straddling}");
        }
    }

    #[test]
    fn an_orthographic_frustum_is_a_box() {
        let frustum = View3D::orthographic(Affine3A::IDENTITY, 5.0, 2.0, 0.1, 50.0).frustum();
        assert_eq!(frustum.planes().len(), 6);
        assert!(frustum.contains(Vec3::new(9.9, 4.9, -49.0)));
        assert!(!frustum.contains(Vec3::new(10.1, 0.0, -1.0)));
        assert!(!frustum.contains(Vec3::new(0.0, 5.1, -1.0)));
        assert!(!frustum.contains(Vec3::new(0.0, 0.0, -50.1)));
        assert!(!frustum.contains(Vec3::new(0.0, 0.0, 0.0)), "before near");
    }

    #[test]
    fn a_non_finite_box_is_never_seen() {
        let frustum = perspective(None);
        let broken = Aabb3 {
            min: Vec3::splat(f32::NAN),
            max: Vec3::splat(f32::NAN),
        };
        assert!(!frustum.intersects(&broken));
    }
}
