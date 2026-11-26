//! Snapping down: a character that stood on a floor reaches down for one.

use voltra_scene::Transform;

use super::slide::Mover;
use super::{Slide, TOUCH};

impl Mover<'_> {
    /// Reaches down by `reach` for a floor, and stands on it.
    pub(super) fn snap(&self, at: &mut Transform, slide: &mut Slide, reach: f32) {
        let Some(hit) = self.cast(at, -self.up * reach) else {
            return;
        };
        if hit.depth > TOUCH || !self.is_floor(hit.normal) {
            return;
        }
        at.translation -= self.up * hit.distance;
        self.touch(slide, hit);
    }
}
