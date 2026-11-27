//! Shaking: an offset and a turn that grow with the square of a trauma that
//! fades.

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
    /// The most it turns either way at full trauma, in radians: rotational
    /// trauma, through the same noise. `0` — the default, and not written —
    /// does not turn.
    #[serde(skip_serializing_if = "is_zero")]
    pub max_angle: f32,
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
    /// The turn last frame added, taken off before the next.
    applied_angle: f32,
}

fn is_zero(value: &f32) -> bool {
    *value == 0.0
}

impl Default for CameraShake {
    fn default() -> Self {
        Self {
            max_offset: Vec2::splat(0.3),
            max_angle: 0.0,
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

    /// The offset and the turn at this trauma and time: one noise, a
    /// channel each, phased apart.
    fn offset(&self) -> (Vec2, f32) {
        let strength = self.trauma * self.trauma;
        let t = self.state.time * self.frequency.max(0.0) * std::f32::consts::TAU;
        (
            Vec2::new(noise(t, 0.0), noise(t, 1.7)) * self.max_offset * strength,
            noise(t, 3.9) * self.max_angle * strength,
        )
    }

    /// The offset this shake has added to its entity's translation now.
    pub fn applied_offset(&self) -> Vec2 {
        self.state.applied
    }

    /// The turn this shake has added to its entity's rotation now, in
    /// radians.
    pub fn applied_angle(&self) -> f32 {
        self.state.applied_angle
    }
}

/// Smooth noise in `-1..=1`: three sines at frequencies that do not repeat
/// together, phased by `seed`.
fn noise(t: f32, seed: f32) -> f32 {
    0.5 * (t + seed).sin()
        + 0.3 * (t * 2.31 + seed * 3.1).sin()
        + 0.2 * (t * 4.17 + seed * 5.3).sin()
}

/// Takes last frame's offsets and turns off, so what follows sees where
/// each rests.
pub(super) fn settle(world: &mut World) {
    let shaking: Vec<(Entity, Vec2, f32)> = world
        .query::<CameraShake>()
        .filter(|(_, shake)| shake.state.applied != Vec2::ZERO || shake.state.applied_angle != 0.0)
        .map(|(entity, shake)| (entity, shake.state.applied, shake.state.applied_angle))
        .collect();
    for (entity, applied, angle) in shaking {
        if let Some(transform) = world.get_mut::<Transform>(entity) {
            transform.translation -= applied;
            transform.rotation -= angle;
        }
        if let Some(shake) = world.get_mut::<CameraShake>(entity) {
            shake.state.applied = Vec2::ZERO;
            shake.state.applied_angle = 0.0;
        }
    }
}

/// Runs every shake `delta` seconds and adds its offset and turn.
pub(super) fn update(world: &mut World, delta: f32) {
    let offsets: Vec<(Entity, Vec2, f32)> = world
        .query_mut::<CameraShake>()
        .filter_map(|(entity, shake)| {
            if shake.trauma <= 0.0 {
                return None;
            }
            shake.state.time += delta;
            let (offset, angle) = shake.offset();
            shake.trauma = (shake.trauma - shake.decay.max(0.0) * delta).max(0.0);
            let offset = if offset.is_finite() {
                offset
            } else {
                Vec2::ZERO
            };
            let angle = if angle.is_finite() { angle } else { 0.0 };
            shake.state.applied = offset;
            shake.state.applied_angle = angle;
            Some((entity, offset, angle))
        })
        .collect();
    for (entity, offset, angle) in offsets {
        match world.get_mut::<Transform>(entity) {
            Some(transform) => {
                transform.translation += offset;
                transform.rotation += angle;
            }
            None => {
                if let Some(shake) = world.get_mut::<CameraShake>(entity) {
                    shake.state.applied = Vec2::ZERO;
                    shake.state.applied_angle = 0.0;
                }
            }
        }
    }
}
