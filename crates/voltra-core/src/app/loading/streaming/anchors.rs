//! Where a streamed world is kept loaded around.

use voltra_render::glam::Vec2;
use voltra_scene::{hierarchy, Camera, StreamingAnchor};

use crate::app::App;

impl App {
    /// Where every anchor stands and how far it reaches: the
    /// [`StreamingAnchor`]s, or the active cameras when there are none.
    pub(super) fn streaming_anchors(&self, default_radius: f32) -> Vec<(Vec2, f32)> {
        let at = |entity| hierarchy::world_transform(&self.world, entity).translation;
        let anchors: Vec<(Vec2, f32)> = self
            .world
            .query::<StreamingAnchor>()
            .map(|(entity, anchor)| {
                let radius = anchor.radius.unwrap_or(default_radius);
                (
                    at(entity),
                    if radius.is_finite() {
                        radius.max(0.0)
                    } else {
                        0.0
                    },
                )
            })
            .collect();
        if !anchors.is_empty() {
            return anchors;
        }
        self.world
            .query::<Camera>()
            .filter(|(_, camera)| camera.active)
            .map(|(entity, _)| (at(entity), default_radius))
            .collect()
    }
}
