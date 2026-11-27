//! Zooming a following camera so the group it frames fits.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;

use crate::Camera;

/// How a following camera zooms to frame its targets — Cinemachine's
/// Framing Transposer over a Target Group: the camera's [`Camera::size`]
/// eased, at the follower's smoothing, to the size that shows the box
/// around every target plus `margin`, within `min_size..=max_size`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FollowZoom {
    /// Room kept around the targets' box on each side, in world units: the
    /// targets are points, and a player at the edge of the screen is half
    /// off it.
    pub margin: Vec2,
    /// The closest it zooms in: the smallest [`Camera::size`] it sets.
    pub min_size: f32,
    /// The furthest it zooms out: the largest [`Camera::size`] it sets.
    pub max_size: f32,
}

impl Default for FollowZoom {
    fn default() -> Self {
        Self {
            margin: Vec2::splat(2.0),
            min_size: 3.0,
            max_size: 12.0,
        }
    }
}

impl FollowZoom {
    /// The camera size that shows a box of half size `half` — the targets'
    /// box — and the margin around it, through a view of `aspect` turned by
    /// `rotation` radians, within the limits.
    ///
    /// Limits out of order are taken either way round, and both are kept
    /// within what a [`Camera`] can show. A non-finite answer is `None`, and
    /// the camera keeps its size.
    pub fn size_for(&self, half: Vec2, aspect: f32, rotation: f32) -> Option<f32> {
        let half = half.abs() + self.margin.max(Vec2::ZERO);
        // The box seen along the turned view's own axes.
        let (sin, cos) = if rotation.is_finite() {
            rotation.sin_cos()
        } else {
            (0.0, 1.0)
        };
        let along = Vec2::new(
            cos.abs() * half.x + sin.abs() * half.y,
            sin.abs() * half.x + cos.abs() * half.y,
        );
        let aspect = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else {
            1.0
        };
        let wanted = along.y.max(along.x / aspect);
        let (lo, hi) = limits(self.min_size, self.max_size);
        let size = wanted.clamp(lo, hi);
        size.is_finite().then_some(size)
    }
}

/// `min..=max` in order and inside what a camera can show; a non-finite
/// end is the camera's own limit.
fn limits(min: f32, max: f32) -> (f32, f32) {
    let finite = |v: f32, or: f32| if v.is_finite() { v } else { or };
    let (a, b) = (finite(min, Camera::MIN_SIZE), finite(max, Camera::MAX_SIZE));
    let (lo, hi) = (a.min(b), a.max(b));
    (
        lo.clamp(Camera::MIN_SIZE, Camera::MAX_SIZE),
        hi.clamp(Camera::MIN_SIZE, Camera::MAX_SIZE),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_size_fits_the_box_and_its_margin_by_the_tighter_axis() {
        let zoom = FollowZoom {
            margin: Vec2::splat(1.0),
            min_size: 1.0,
            max_size: 100.0,
        };
        // Eight wide (with margin), two tall, on a 2:1 view: width decides.
        assert_eq!(zoom.size_for(Vec2::new(7.0, 1.0), 2.0, 0.0), Some(4.0));
        // Two wide, six tall: height decides.
        assert_eq!(zoom.size_for(Vec2::new(1.0, 5.0), 2.0, 0.0), Some(6.0));
    }

    #[test]
    fn the_size_stays_within_its_limits_taken_either_way_round() {
        let zoom = FollowZoom {
            margin: Vec2::ZERO,
            min_size: 8.0,
            max_size: 3.0,
        };
        assert_eq!(zoom.size_for(Vec2::ZERO, 1.0, 0.0), Some(3.0));
        assert_eq!(zoom.size_for(Vec2::splat(50.0), 1.0, 0.0), Some(8.0));
    }

    #[test]
    fn a_turned_view_fits_the_box_turned() {
        let zoom = FollowZoom {
            margin: Vec2::ZERO,
            min_size: 0.1,
            max_size: 100.0,
        };
        // A quarter turn swaps the axes.
        let size = zoom
            .size_for(Vec2::new(1.0, 5.0), 1.0, std::f32::consts::FRAC_PI_2)
            .expect("finite");
        assert!((size - 5.0).abs() < 1e-5);
        let turned = zoom
            .size_for(Vec2::new(4.0, 0.0), 2.0, std::f32::consts::FRAC_PI_2)
            .expect("finite");
        assert!((turned - 4.0).abs() < 1e-5, "{turned}");
    }
}
