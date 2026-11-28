//! An axis-aligned box: a mesh's local bounds, and an instance's in the
//! world.

use glam::{Affine3A, Vec3};

/// An axis-aligned box, `min` to `max` on every axis.
///
/// What culling tests against the frustum: a mesh keeps its local box, and
/// an instance's world box is that box carried through its model matrix
/// ([`transformed`](Self::transformed)) — loose for a turned box, never
/// tight enough to cull something visible.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb3 {
    /// The corner with every coordinate smallest.
    pub min: Vec3,
    /// The corner with every coordinate largest.
    pub max: Vec3,
}

impl Aabb3 {
    /// The box from `min` to `max`. Swapped coordinates are put in order.
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self {
            min: min.min(max),
            max: min.max(max),
        }
    }

    /// The smallest box holding every one of `points`, or `None` for none.
    pub fn from_points(points: impl IntoIterator<Item = Vec3>) -> Option<Self> {
        let mut points = points.into_iter();
        let first = points.next()?;
        let (min, max) = points.fold((first, first), |(min, max), p| (min.min(p), max.max(p)));
        Some(Self { min, max })
    }

    /// The middle of the box.
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Half the box's size on each axis.
    pub fn half_extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    /// Whether every coordinate is a finite number.
    pub fn is_finite(&self) -> bool {
        self.min.is_finite() && self.max.is_finite()
    }

    /// The world box of this local one under `model`: the box around its
    /// eight corners moved, computed from the centre and the absolute
    /// matrix (Arvo's method) rather than corner by corner.
    pub fn transformed(&self, model: &Affine3A) -> Self {
        let center = model.transform_point3(self.center());
        let half = self.half_extents();
        let m = model.matrix3;
        let extent = m.x_axis.abs() * half.x + m.y_axis.abs() * half.y + m.z_axis.abs() * half.z;
        let extent = Vec3::from(extent);
        Self {
            min: center - extent,
            max: center + extent,
        }
    }
}

#[cfg(test)]
mod tests {
    use glam::Quat;

    use super::*;

    #[test]
    fn a_box_from_points_holds_them_all() {
        let points = [
            Vec3::new(1.0, -2.0, 0.5),
            Vec3::new(-1.0, 3.0, 0.0),
            Vec3::ZERO,
        ];
        let b = Aabb3::from_points(points).expect("three points");
        assert_eq!(b.min, Vec3::new(-1.0, -2.0, 0.0));
        assert_eq!(b.max, Vec3::new(1.0, 3.0, 0.5));
        assert!(Aabb3::from_points([]).is_none());
    }

    #[test]
    fn a_turned_box_is_held_by_its_world_box() {
        let local = Aabb3::new(Vec3::splat(-1.0), Vec3::splat(1.0));
        let model = Affine3A::from_scale_rotation_translation(
            Vec3::new(2.0, 1.0, 1.0),
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_4),
            Vec3::new(10.0, 0.0, -3.0),
        );
        let world = local.transformed(&model);
        for x in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    let corner = model.transform_point3(Vec3::new(x, y, z));
                    assert!(corner.cmpge(world.min - 1e-5).all(), "{corner}");
                    assert!(corner.cmple(world.max + 1e-5).all(), "{corner}");
                }
            }
        }
        // A 2 by 2 square turned 45° is 3·√2 wide at scale 2 in x.
        let width = world.max.x - world.min.x;
        assert!(
            (width - 3.0 * std::f32::consts::SQRT_2).abs() < 1e-4,
            "{width}"
        );
    }
}
