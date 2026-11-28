//! A place a client is sent the world around, in one dimension.

use glam::{Vec2, Vec3};

/// Where a client watches from, or where a replicated entity stands, for
/// interest management: a point of the 2D world or of the 3D one.
///
/// A point sees only entities of its own dimension — a 2D level and a 3D
/// one in the same world are two places, not one seen flat. Both share the
/// server's one grid: a 2D point at its (x, y), a 3D point on the ground
/// plane at its **(x, z)** — Unreal's 2D spatialisation for 3D worlds, so a
/// tall level costs no more cells than a flat one — and the last test of a
/// 3D point is the full 3D distance, so a flying entity overhead is still
/// out of range.
///
/// `From` either vector, so `set_interest_points(peer, vec![Vec2::new(…)])`
/// reads as it did before 3D, and a 3D game passes `Vec3`s the same way.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InterestPoint {
    /// A point of the 2D world.
    D2(Vec2),
    /// A point of the 3D world: Y up, metres.
    D3(Vec3),
}

impl From<Vec2> for InterestPoint {
    fn from(at: Vec2) -> Self {
        Self::D2(at)
    }
}

impl From<Vec3> for InterestPoint {
    fn from(at: Vec3) -> Self {
        Self::D3(at)
    }
}

impl InterestPoint {
    /// Whether every coordinate is finite: a point that is not is never
    /// near anything.
    pub fn is_finite(&self) -> bool {
        match self {
            Self::D2(at) => at.is_finite(),
            Self::D3(at) => at.is_finite(),
        }
    }

    /// Whether it is in the 3D world.
    pub fn is_3d(&self) -> bool {
        matches!(self, Self::D3(_))
    }

    /// Where it falls on the grid's plane: a 2D point's (x, y), a 3D
    /// point's (x, z).
    pub(crate) fn plane(&self) -> Vec2 {
        match *self {
            Self::D2(at) => at,
            Self::D3(at) => Vec2::new(at.x, at.z),
        }
    }

    /// The coordinate off the grid's plane: a 3D point's height, nothing
    /// for a 2D one — so the squared distance of two points of one
    /// dimension is their plane's plus this one's, exactly the 2D one in 2D.
    pub(crate) fn height(&self) -> f32 {
        match self {
            Self::D2(_) => 0.0,
            Self::D3(at) => at.y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_3d_point_lies_on_the_ground_plane_by_its_x_and_z() {
        let point = InterestPoint::from(Vec3::new(1.0, 2.0, 3.0));
        assert!(point.is_3d());
        assert_eq!(point.plane(), Vec2::new(1.0, 3.0));
        assert_eq!(point.height(), 2.0);
        let flat = InterestPoint::from(Vec2::new(1.0, 2.0));
        assert!(!flat.is_3d());
        assert_eq!((flat.plane(), flat.height()), (Vec2::new(1.0, 2.0), 0.0));
        assert!(!InterestPoint::D3(Vec3::new(0.0, f32::NAN, 0.0)).is_finite());
    }
}
