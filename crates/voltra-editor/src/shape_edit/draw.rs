//! The handles of the polygon being edited, as overlay lines.

use voltra_core::egui::Response;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::hierarchy::world_matrix;

use super::handle::{self, Handle};
use super::{polygon, viewport_of, ShapeEdit};

/// A point's square.
const POINT_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 0.95];
/// The point or midpoint under the pointer, or being dragged.
const HOT_COLOR: [f32; 4] = [1.0, 0.85, 0.3, 1.0];
/// A midpoint's square: the caster outline's violet, faded.
const MIDPOINT_COLOR: [f32; 4] = [0.6, 0.45, 1.0, 0.7];
/// Outline thickness, in pixels; the other overlays' width.
const WIDTH: f32 = 1.5;

impl ShapeEdit {
    /// Pushes the handles of `entity`'s polygon: a square at each point, a
    /// smaller one at each edge's middle, the one under the pointer lit.
    pub fn draw(&self, scene: &Response, frame: &mut UiFrame<'_>, entity: Entity) {
        let viewport = viewport_of(scene);
        if viewport.x <= 0.0 || viewport.y <= 0.0 {
            return;
        }
        let Some((points, closed)) = polygon(frame.world, entity) else {
            return;
        };
        let matrix = world_matrix(frame.world, entity);
        let placed: Vec<Vec2> = points.iter().map(|p| matrix.transform_point2(*p)).collect();
        // World units per screen pixel, so the squares keep their size on
        // screen whatever the zoom.
        let per_pixel = frame.camera.half_extents().x * 2.0 / viewport.x;
        let hot = self.drag.map(Handle::Point).or(self.hovered);

        let mut squares = Vec::new();
        for index in 0..handle::edge_count(placed.len(), closed) {
            let color = if hot == Some(Handle::Edge(index)) {
                HOT_COLOR
            } else {
                MIDPOINT_COLOR
            };
            let at = handle::midpoint(&placed, index);
            squares.push((at, handle::MIDPOINT_HALF * per_pixel, color));
        }
        for (index, at) in placed.iter().enumerate() {
            let color = if hot == Some(Handle::Point(index)) {
                HOT_COLOR
            } else {
                POINT_COLOR
            };
            squares.push((*at, handle::POINT_HALF * per_pixel, color));
        }

        let lines = frame.lines();
        for (at, half, color) in squares {
            let corners = [
                at + Vec2::new(-half, -half),
                at + Vec2::new(half, -half),
                at + Vec2::new(half, half),
                at + Vec2::new(-half, half),
            ];
            for i in 0..4 {
                lines.push(corners[i], corners[(i + 1) % 4], WIDTH, color);
            }
        }
    }
}
