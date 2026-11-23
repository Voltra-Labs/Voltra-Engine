//! The closest points of two segments: Box2D v3's `b2SegmentDistance`.

use voltra_render::glam::Vec2;

/// Where two segments come nearest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SegmentDistance {
    /// On the first segment.
    pub closest1: Vec2,
    /// On the second.
    pub closest2: Vec2,
    /// How far along the first `closest1` is, `0` at `p1` to `1` at `q1`.
    pub fraction1: f32,
    pub fraction2: f32,
    pub distance_squared: f32,
}

/// The closest points of `p1 → q1` and `p2 → q2`.
///
/// Either segment may have no length, and then it is a point; parallel ones
/// answer with the first segment's start clamped onto the second.
pub(crate) fn segment_distance(p1: Vec2, q1: Vec2, p2: Vec2, q2: Vec2) -> SegmentDistance {
    let (d1, d2) = (q1 - p1, q2 - p2);
    let r = p1 - p2;
    let (dd1, dd2) = (d1.dot(d1), d2.dot(d2));
    let (rd1, rd2) = (r.dot(d1), r.dot(d2));
    let tiny = f32::EPSILON * f32::EPSILON;

    let (f1, f2) = if dd1 < tiny || dd2 < tiny {
        if dd1 >= tiny {
            ((-rd1 / dd1).clamp(0.0, 1.0), 0.0)
        } else if dd2 >= tiny {
            (0.0, (rd2 / dd2).clamp(0.0, 1.0))
        } else {
            (0.0, 0.0)
        }
    } else {
        let d12 = d1.dot(d2);
        let denominator = dd1 * dd2 - d12 * d12;
        let mut f1 = if denominator != 0.0 {
            ((d12 * rd2 - rd1 * dd2) / denominator).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mut f2 = (d12 * f1 + rd2) / dd2;
        // Clamping the second means the first has to be found again.
        if f2 < 0.0 {
            f2 = 0.0;
            f1 = (-rd1 / dd1).clamp(0.0, 1.0);
        } else if f2 > 1.0 {
            f2 = 1.0;
            f1 = ((d12 - rd1) / dd1).clamp(0.0, 1.0);
        }
        (f1, f2)
    };

    let closest1 = p1 + d1 * f1;
    let closest2 = p2 + d2 * f2;
    SegmentDistance {
        closest1,
        closest2,
        fraction1: f1,
        fraction2: f2,
        distance_squared: closest1.distance_squared(closest2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossing_segments_meet() {
        let result = segment_distance(
            Vec2::new(-1.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, -1.0),
            Vec2::new(0.0, 1.0),
        );
        assert!(result.distance_squared < 1e-10, "{result:?}");
        assert!((result.fraction1 - 0.5).abs() < 1e-6);
    }

    #[test]
    fn an_end_nearest_the_other_segments_middle() {
        let result = segment_distance(
            Vec2::new(-1.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.5, 2.0),
            Vec2::new(0.5, 1.0),
        );
        assert!((result.distance_squared - 1.0).abs() < 1e-6, "{result:?}");
        assert_eq!(result.fraction2, 1.0);
    }

    #[test]
    fn a_point_against_a_segment() {
        let result = segment_distance(
            Vec2::new(3.0, 1.0),
            Vec2::new(3.0, 1.0),
            Vec2::ZERO,
            Vec2::new(2.0, 0.0),
        );
        assert!((result.distance_squared - 2.0).abs() < 1e-6, "{result:?}");
    }
}
