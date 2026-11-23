//! The points of a convex polygon collider: at most eight, convex and
//! counter-clockwise, made so by construction.
//!
//! Whatever a scene file or an editor hands over is turned into its convex
//! hull — duplicates welded, points on a straight edge dropped, the rest
//! wound counter-clockwise — so nothing downstream checks again. Eight is
//! Box2D's `B2_MAX_POLYGON_VERTICES`, and the reason is the same: a fixed
//! array keeps [`Collider`](crate::Collider) `Copy` and costs no allocation
//! per contact, and a convex outline that needs more than eight points is a
//! circle or a capsule in disguise.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use voltra_render::glam::Vec2;

/// Why a set of points is not a polygon collider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolygonError {
    /// Fewer than three points that are not on one line: no area.
    TooFew,
    /// The hull needs more corners than a polygon collider holds.
    TooMany(usize),
}

impl std::fmt::Display for PolygonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooFew => write!(f, "a polygon needs three points that are not on one line"),
            Self::TooMany(count) => write!(
                f,
                "a polygon holds at most {} points, and this hull has {count}",
                ConvexPolygon::MAX_POINTS
            ),
        }
    }
}

impl std::error::Error for PolygonError {}

/// A convex polygon, counter-clockwise, in the collider's own frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConvexPolygon {
    points: [Vec2; ConvexPolygon::MAX_POINTS],
    count: u8,
}

/// Below this sine an angle at a corner is a straight line.
const COLLINEAR: f32 = 1e-5;

/// Points nearer than this share of the hull's size are one point.
const WELD: f32 = 1e-5;

impl ConvexPolygon {
    /// The most corners a polygon collider has.
    pub const MAX_POINTS: usize = 8;

    /// The convex hull of `points`.
    ///
    /// Any order, any number, duplicates and points inside allowed; what is
    /// kept is the outline, counter-clockwise. Fails when that outline has no
    /// area or needs more than [`Self::MAX_POINTS`] corners — use
    /// [`Self::enclosing`] to reduce one instead.
    pub fn new(points: &[Vec2]) -> Result<Self, PolygonError> {
        let hull = hull(points);
        if hull.len() < 3 {
            return Err(PolygonError::TooFew);
        }
        if hull.len() > Self::MAX_POINTS {
            return Err(PolygonError::TooMany(hull.len()));
        }
        Ok(Self::from_hull(&hull))
    }

    /// The convex hull of `points`, cut down to [`Self::MAX_POINTS`] corners
    /// by growing it, so it still covers every point.
    ///
    /// What a sprite's traced silhouette becomes: its hull can have dozens
    /// of corners, and a collider that cuts into what is drawn reads as the
    /// physics being wrong. Each cut removes the edge whose two neighbours,
    /// extended until they meet, add the least area.
    pub fn enclosing(points: &[Vec2]) -> Result<Self, PolygonError> {
        let mut hull = hull(points);
        if hull.len() < 3 {
            return Err(PolygonError::TooFew);
        }
        while hull.len() > Self::MAX_POINTS {
            if !grow_out_one_edge(&mut hull) {
                // Cannot happen past four corners: the turns of a convex
                // outline add up to a full turn, so some neighbouring pair
                // turns by less than half of one. Kept as a stop, not a loop.
                return Err(PolygonError::TooMany(hull.len()));
            }
        }
        Ok(Self::from_hull(&hull))
    }

    /// A box of `half_extents` as a polygon, counter-clockwise from
    /// `(+hx, +hy)` — what a box collider would be as one.
    pub fn rectangle(half_extents: Vec2) -> Result<Self, PolygonError> {
        let (x, y) = (half_extents.x, half_extents.y);
        Self::new(&[
            Vec2::new(x, y),
            Vec2::new(-x, y),
            Vec2::new(-x, -y),
            Vec2::new(x, -y),
        ])
    }

    fn from_hull(hull: &[Vec2]) -> Self {
        let mut points = [Vec2::ZERO; Self::MAX_POINTS];
        points[..hull.len()].copy_from_slice(hull);
        Self {
            points,
            count: hull.len() as u8,
        }
    }

    /// The corners, counter-clockwise.
    pub fn points(&self) -> &[Vec2] {
        &self.points[..self.count as usize]
    }

    /// The signed area, positive for the counter-clockwise
    /// winding every polygon here has.
    pub fn area(&self) -> f32 {
        signed_area(self.points())
    }
}

/// The convex hull, counter-clockwise, of the finite points among `points`:
/// Andrew's monotone chain, with near-duplicates welded and straight
/// corners dropped.
fn hull(points: &[Vec2]) -> Vec<Vec2> {
    let mut sorted: Vec<Vec2> = points.iter().copied().filter(|p| p.is_finite()).collect();
    sorted.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    let Some((&first, _)) = sorted.split_first() else {
        return Vec::new();
    };
    let (min, max) = sorted
        .iter()
        .fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    let weld = (max - min).length() * WELD;
    sorted.dedup_by(|a, b| a.distance(*b) <= weld);
    if sorted.len() < 3 {
        return sorted;
    }

    // A corner turning left by less than this sine is no corner.
    let turns_left = |o: Vec2, a: Vec2, b: Vec2| {
        let (u, v) = (a - o, b - o);
        u.perp_dot(v) > COLLINEAR * u.length() * v.length()
    };
    let mut chain: Vec<Vec2> = Vec::with_capacity(sorted.len() * 2);
    for &point in &sorted {
        while chain.len() >= 2 && !turns_left(chain[chain.len() - 2], chain[chain.len() - 1], point)
        {
            chain.pop();
        }
        chain.push(point);
    }
    // The upper half, back from the right end; its last point is the first.
    let lower = chain.len() + 1;
    for &point in sorted.iter().rev().skip(1) {
        while chain.len() >= lower
            && !turns_left(chain[chain.len() - 2], chain[chain.len() - 1], point)
        {
            chain.pop();
        }
        chain.push(point);
    }
    chain.pop();
    if chain.len() < 3 || signed_area(&chain) <= 0.0 {
        return chain.into_iter().take(2).collect();
    }
    chain
}

/// Replaces the edge whose neighbours meet with the least area added by
/// their meeting point. `false` when no edge's neighbours meet.
fn grow_out_one_edge(hull: &mut Vec<Vec2>) -> bool {
    let n = hull.len();
    let mut best: Option<(usize, Vec2, f32)> = None;
    for i in 0..n {
        let (prev, a, b, next) = (
            hull[(i + n - 1) % n],
            hull[i],
            hull[(i + 1) % n],
            hull[(i + 2) % n],
        );
        let (before, after) = (a - prev, next - b);
        let turn = before.perp_dot(after);
        // The two lines meet beyond the edge only while together they turn
        // by less than half a turn.
        if turn <= 0.0 {
            continue;
        }
        let s = (b - a).perp_dot(after) / turn;
        let meet = a + before * s;
        let added = 0.5 * (a - meet).perp_dot(b - meet).abs();
        if best.is_none_or(|(_, _, area)| added < area) {
            best = Some((i, meet, added));
        }
    }
    let Some((i, meet, _)) = best else {
        return false;
    };
    hull[i] = meet;
    hull.remove((i + 1) % n);
    true
}

fn signed_area(points: &[Vec2]) -> f32 {
    let n = points.len();
    0.5 * (0..n)
        .map(|i| points[i].perp_dot(points[(i + 1) % n]))
        .sum::<f32>()
}

impl Serialize for ConvexPolygon {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.points().serialize(serializer)
    }
}

/// A list of points, hulled on the way in: a file that lists a concave
/// outline or a repeated point loads as the polygon it describes, and one
/// that describes none is an error naming why.
impl<'de> Deserialize<'de> for ConvexPolygon {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let points = Vec::<Vec2>::deserialize(deserializer)?;
        Self::new(&points).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> [Vec2; 4] {
        [
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
        ]
    }

    #[test]
    fn points_in_any_order_come_out_counter_clockwise() {
        let mut clockwise = square();
        clockwise.reverse();
        let polygon = ConvexPolygon::new(&clockwise).expect("a square");

        assert_eq!(polygon.points().len(), 4);
        assert!((polygon.area() - 4.0).abs() < 1e-5, "{polygon:?}");
    }

    #[test]
    fn inner_repeated_and_straight_points_are_dropped() {
        let mut points = square().to_vec();
        points.push(Vec2::ZERO);
        points.push(Vec2::new(1.0, 1.0));
        points.push(Vec2::new(0.0, 1.0));
        let polygon = ConvexPolygon::new(&points).expect("still a square");

        assert_eq!(polygon.points().len(), 4, "{polygon:?}");
    }

    #[test]
    fn points_on_a_line_are_no_polygon() {
        let line = [Vec2::ZERO, Vec2::X, Vec2::X * 2.0];
        assert_eq!(ConvexPolygon::new(&line), Err(PolygonError::TooFew));
        assert_eq!(ConvexPolygon::new(&[Vec2::ZERO]), Err(PolygonError::TooFew));
        assert_eq!(
            ConvexPolygon::new(&[Vec2::NAN, Vec2::X, Vec2::Y]),
            Err(PolygonError::TooFew)
        );
    }

    fn circle(sides: usize) -> Vec<Vec2> {
        (0..sides)
            .map(|i| Vec2::from_angle(i as f32 / sides as f32 * std::f32::consts::TAU))
            .collect()
    }

    #[test]
    fn a_hull_of_more_than_eight_is_refused_by_new() {
        assert_eq!(
            ConvexPolygon::new(&circle(12)),
            Err(PolygonError::TooMany(12))
        );
    }

    #[test]
    fn enclosing_cuts_a_hull_to_eight_and_still_covers_every_point() {
        let points = circle(40);
        let polygon = ConvexPolygon::enclosing(&points).expect("a polygon");

        assert_eq!(polygon.points().len(), ConvexPolygon::MAX_POINTS);
        let corners = polygon.points();
        for point in &points {
            for i in 0..corners.len() {
                let (a, b) = (corners[i], corners[(i + 1) % corners.len()]);
                assert!(
                    (b - a).perp_dot(*point - a) >= -1e-4,
                    "{point:?} is outside {polygon:?}"
                );
            }
        }
        // Grown, not bloated: within a few per cent of the circle.
        let area = polygon.area();
        assert!(
            area < std::f32::consts::PI * 1.12,
            "{area} for a unit circle"
        );
    }

    #[test]
    fn a_polygon_round_trips_through_ron_as_a_list() {
        let polygon = ConvexPolygon::new(&square()).expect("a square");
        let text = ron::to_string(&polygon).expect("serialise");
        assert!(text.starts_with('['), "{text}");
        let back: ConvexPolygon = ron::from_str(&text).expect("deserialise");
        assert_eq!(back, polygon);
    }

    #[test]
    fn a_file_listing_no_polygon_is_an_error_saying_so() {
        let error = ron::from_str::<ConvexPolygon>("[(0,0),(1,0)]").expect_err("two points");
        assert!(error.to_string().contains("three points"), "{error}");
    }
}
