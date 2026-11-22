//! Shaking: an offset that grows with the square of a trauma that fades.

use serde::{Deserialize, Serialize};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use crate::Transform;

/// Shakes the entity — a camera, almost always — by its trauma.
///
/// Squirrel Eiserloh's model ("Juicing Your Cameras With Math", GDC 2016):
/// a game adds trauma when something hits, it fades on its own, and the
/// offset is the trauma *squared* times smooth noise, so small knocks
/// barely show and big ones shake hard. Noise, not random jumps, so the
/// shake is a motion rather than a flicker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraShake {
    /// The most it moves either way at full trauma, in world units.
    pub max_offset: Vec2,
    /// How fast the shake moves, roughly in shakes a second.
    pub frequency: f32,
    /// Trauma lost per second.
    pub decay: f32,
    /// Zero still, one shaking as hard as it does. Never saved: a shake is
    /// something that happens.
    #[serde(skip)]
    pub trauma: f32,
    /// Where the shake is between frames. Never saved.
    #[serde(skip)]
    pub state: ShakeState,
}

/// A shake's memory between frames.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ShakeState {
    time: f32,
    /// The offset last frame added, taken off before the next.
    applied: Vec2,
}

impl Default for CameraShake {
    fn default() -> Self {
        Self {
            max_offset: Vec2::splat(0.3),
            frequency: 15.0,
            decay: 1.2,
            trauma: 0.0,
            state: ShakeState::default(),
        }
    }
}

impl CameraShake {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "CameraShake";

    /// Adds `amount` of trauma, up to one.
    pub fn add_trauma(&mut self, amount: f32) {
        if amount.is_finite() {
            self.trauma = (self.trauma + amount).clamp(0.0, 1.0);
        }
    }

    /// The offset at this trauma and time.
    fn offset(&self) -> Vec2 {
        let strength = self.trauma * self.trauma;
        let t = self.state.time * self.frequency.max(0.0) * std::f32::consts::TAU;
        Vec2::new(noise(t, 0.0), noise(t, 1.7)) * self.max_offset * strength
    }
}

/// Smooth noise in `-1..=1`: three sines at frequencies that do not repeat
/// together, phased by `seed`.
fn noise(t: f32, seed: f32) -> f32 {
    0.5 * (t + seed).sin()
        + 0.3 * (t * 2.31 + seed * 3.1).sin()
        + 0.2 * (t * 4.17 + seed * 5.3).sin()
}

/// Takes last frame's offsets off, so what follows sees where each rests.
pub(super) fn settle(world: &mut World) {
    let shaking: Vec<(Entity, Vec2)> = world
        .query::<CameraShake>()
        .filter(|(_, shake)| shake.state.applied != Vec2::ZERO)
        .map(|(entity, shake)| (entity, shake.state.applied))
        .collect();
    for (entity, applied) in shaking {
        if let Some(transform) = world.get_mut::<Transform>(entity) {
            transform.translation -= applied;
        }
        if let Some(shake) = world.get_mut::<CameraShake>(entity) {
            shake.state.applied = Vec2::ZERO;
        }
    }
}

/// Runs every shake `delta` seconds and adds its offset.
pub(super) fn update(world: &mut World, delta: f32) {
    let offsets: Vec<(Entity, Vec2)> = world
        .query_mut::<CameraShake>()
        .filter_map(|(entity, shake)| {
            if shake.trauma <= 0.0 {
                return None;
            }
            shake.state.time += delta;
            let offset = shake.offset();
            shake.trauma = (shake.trauma - shake.decay.max(0.0) * delta).max(0.0);
            let offset = if offset.is_finite() {
                offset
            } else {
                Vec2::ZERO
            };
            shake.state.applied = offset;
            Some((entity, offset))
        })
        .collect();
    for (entity, offset) in offsets {
        match world.get_mut::<Transform>(entity) {
            Some(transform) => transform.translation += offset,
            None => {
                if let Some(shake) = world.get_mut::<CameraShake>(entity) {
                    shake.state.applied = Vec2::ZERO;
                }
            }
        }
    }
}
