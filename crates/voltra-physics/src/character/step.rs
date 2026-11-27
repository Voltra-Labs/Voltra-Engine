//! Stepping up: over a seam between two colliders, or onto a ledge low
//! enough to walk onto.
//!
//! A box sliding along a floor made of several boxes meets the next one's
//! corner level with its own bottom, and the cast — rightly, for a box —
//! calls it a wall. Every character controller answers it the same way:
//! Unity's `stepOffset`, PhysX's step climbing. Lift by the step height (as
//! far as the room above allows), move across from there, and come back
//! down: kept if it went further than the blocked move and landed on a
//! floor.

use voltra_render::glam::Vec2;
use voltra_scene::Transform2D;

use crate::query::ShapeHit;

use super::slide::Mover;
use super::TOUCH;

/// Where a step leaves the character, and the floor it landed on.
pub(super) struct Step {
    pub at: Transform2D,
    /// The part of the motion the step covered, across.
    pub covered: Vec2,
    pub landing: ShapeHit,
}

impl Mover<'_> {
    /// Tries stepping `across` — the motion less its part along `up` — from
    /// `at`, up to `height` high. `None` when there is no room to lift, no
    /// progress from up there, or no floor to come down on.
    pub(super) fn step_up(&self, at: &Transform2D, across: Vec2, height: f32) -> Option<Step> {
        let length = across.length();
        if height <= 0.0 || length <= TOUCH {
            return None;
        }
        let lift = match self.cast(at, self.up * height) {
            Some(hit) if hit.depth > TOUCH => return None,
            Some(hit) => hit.distance,
            None => height,
        };
        if lift <= TOUCH {
            return None;
        }
        let mut lifted = *at;
        lifted.translation += self.up * lift;

        let direction = across / length;
        let advance = match self.cast(&lifted, across) {
            Some(hit) if hit.depth > TOUCH => return None,
            Some(hit) => hit.distance,
            None => length,
        };
        if advance <= TOUCH {
            return None;
        }
        lifted.translation += direction * advance;

        let landing = self.cast(&lifted, -self.up * lift)?;
        if landing.depth > TOUCH || !self.is_floor(landing.normal) {
            return None;
        }
        lifted.translation -= self.up * landing.distance;
        Some(Step {
            at: lifted,
            covered: direction * advance,
            landing,
        })
    }
}
