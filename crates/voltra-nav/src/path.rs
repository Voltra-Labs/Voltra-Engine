//! A path an agent walks, a waypoint at a time.

use voltra_render::glam::Vec2;

/// Waypoints from where an agent is to where it is going, and which one it
/// is heading for.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Path {
    points: Vec<Vec2>,
    next: usize,
}

impl Path {
    /// A path through `points`, heading for the first after the start.
    pub fn new(points: Vec<Vec2>) -> Self {
        let next = usize::from(points.len() > 1);
        Self { points, next }
    }

    /// Every waypoint, the start first.
    pub fn points(&self) -> &[Vec2] {
        &self.points
    }

    /// Where it ends.
    pub fn end(&self) -> Option<Vec2> {
        self.points.last().copied()
    }

    /// Its length along the waypoints, in world units.
    pub fn length(&self) -> f32 {
        self.points.windows(2).map(|w| w[0].distance(w[1])).sum()
    }

    /// The waypoint an agent at `at` should head for, moving on past every
    /// one within `reach` of it; `None` once the last is reached.
    pub fn next_point(&mut self, at: Vec2, reach: f32) -> Option<Vec2> {
        while let Some(&point) = self.points.get(self.next) {
            if point.distance(at) > reach.max(0.0) {
                return Some(point);
            }
            self.next += 1;
        }
        None
    }

    /// Whether every waypoint has been reached.
    pub fn is_finished(&self) -> bool {
        self.next >= self.points.len()
    }
}
