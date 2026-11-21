//! Which point or edge of a polygon is under the pointer, and the edits a
//! handle makes to the list of points.
//!
//! Hit-tested in **screen pixels**, as the gizmo is: handles are drawn at a
//! constant size on screen, so the target must not grow and shrink with the
//! zoom while the picture stays put.

use voltra_render::glam::Vec2;

/// Half-side of a point's square, in pixels.
pub const POINT_HALF: f32 = 4.5;

/// Half-side of an edge's midpoint square, in pixels: smaller, because it
/// adds a point rather than being one.
pub const MIDPOINT_HALF: f32 = 3.0;

/// How far from a handle's centre still grabs it, in pixels. Wider than the
/// square, as every editor pads a small target.
pub const GRAB_RADIUS: f32 = 8.0;

/// A handle of a polygon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    /// The point at this index; dragging it moves it.
    Point(usize),
    /// The middle of the edge from this point to the next; dragging it adds
    /// a point there.
    Edge(usize),
}

/// The edges of `count` points: one fewer than points for an open line, as
/// many for an outline (the last joins the first).
pub fn edge_count(count: usize, closed: bool) -> usize {
    match (count, closed) {
        (0 | 1, _) => 0,
        (2, _) | (_, false) => count - 1,
        (_, true) => count,
    }
}

/// The middle of edge `index` of `points`.
pub fn midpoint(points: &[Vec2], index: usize) -> Vec2 {
    (points[index] + points[(index + 1) % points.len()]) * 0.5
}

/// The handle nearest `cursor` within [`GRAB_RADIUS`], points before
/// midpoints, given the points `on_screen`. `None` when nothing is that
/// close.
pub fn handle_at(cursor: Vec2, on_screen: &[Vec2], closed: bool) -> Option<Handle> {
    let nearest = |candidates: &mut dyn Iterator<Item = (usize, Vec2)>| {
        candidates
            .map(|(index, at)| (index, at.distance(cursor)))
            .filter(|(_, distance)| *distance <= GRAB_RADIUS)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    };
    if let Some(index) = nearest(&mut on_screen.iter().copied().enumerate()) {
        return Some(Handle::Point(index));
    }
    let edges = edge_count(on_screen.len(), closed);
    nearest(&mut (0..edges).map(|index| (index, midpoint(on_screen, index)))).map(Handle::Edge)
}

/// The fewest points the shape keeps: two for a line, three for an outline.
pub fn minimum(closed: bool) -> usize {
    if closed {
        3
    } else {
        2
    }
}

/// Adds a point in the middle of edge `index`; returns where it went.
pub fn insert(points: &mut Vec<Vec2>, index: usize) -> usize {
    let at = midpoint(points, index);
    points.insert(index + 1, at);
    index + 1
}

/// Removes point `index` if the shape keeps its minimum without it; returns
/// whether it went.
pub fn remove(points: &mut Vec<Vec2>, index: usize, closed: bool) -> bool {
    let removable = index < points.len() && points.len() > minimum(closed);
    if removable {
        points.remove(index);
    }
    removable
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<Vec2> {
        vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 100.0),
            Vec2::new(0.0, 100.0),
        ]
    }

    #[test]
    fn a_point_is_grabbed_within_its_radius_and_wins_over_an_edge() {
        let points = square();
        assert_eq!(
            handle_at(Vec2::new(103.0, 4.0), &points, true),
            Some(Handle::Point(1))
        );
        assert_eq!(handle_at(Vec2::new(50.0, 50.0), &points, true), None);
    }

    #[test]
    fn an_edge_is_grabbed_at_its_middle_and_an_open_line_has_no_closing_edge() {
        let points = square();
        assert_eq!(
            handle_at(Vec2::new(52.0, 98.0), &points, true),
            Some(Handle::Edge(2))
        );
        // The edge from the last point back to the first.
        assert_eq!(
            handle_at(Vec2::new(1.0, 50.0), &points, true),
            Some(Handle::Edge(3))
        );
        assert_eq!(handle_at(Vec2::new(1.0, 50.0), &points, false), None);
    }

    #[test]
    fn inserting_puts_the_new_point_between_the_edge_s_ends() {
        let mut points = square();
        let at = insert(&mut points, 3);
        assert_eq!(at, 4);
        assert_eq!(points[4], Vec2::new(0.0, 50.0));
        assert_eq!(points.len(), 5);
    }

    #[test]
    fn removing_stops_at_the_shape_s_minimum() {
        let mut outline = square();
        assert!(remove(&mut outline, 0, true));
        assert!(
            !remove(&mut outline, 0, true),
            "three is an outline's least"
        );
        let mut line = vec![Vec2::ZERO, Vec2::X, Vec2::Y];
        assert!(remove(&mut line, 2, false));
        assert!(!remove(&mut line, 0, false), "two is a line's least");
        assert!(!remove(&mut line, 9, false), "out of range");
    }

    #[test]
    fn edges_are_counted_by_shape() {
        assert_eq!(edge_count(4, true), 4);
        assert_eq!(edge_count(4, false), 3);
        assert_eq!(edge_count(2, true), 1, "two points close into one edge");
        assert_eq!(edge_count(1, true), 0);
    }
}
