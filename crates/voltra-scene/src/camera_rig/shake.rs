//! Shaking: an offset and a turn that grow with the square of a trauma that
//! fades.

use serde::{Deserialize, Serialize};
use voltra_ecs::World;
use voltra_render::glam::Vec2;

/// Shakes the view of the camera on its entity by its trauma.
///
/// The view, not the entity (Cinemachine's noise is a step after the
/// camera is placed, the same way): the offset — along the view's own
/// axes — and the turn are added where the view is worked out, and the
/// transform stays where the game, a follower or a tween put it. On an
/// entity without a [`Camera2D`](crate::Camera2D) there is no view to shake.
///
/// Squirrel Eiserloh's model ("Juicing Your Cameras With Math", GDC 2016):
/// a game adds trauma when something hits, it fades on its own, and the
/// offset is the trauma *squared* times smooth noise, so small knocks
/// barely show and big ones shake hard. Noise, not random jumps, so the
/// shake is a motion rather than a flicker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraShake {
    /// The most it moves either way at full trauma, in world units along
    /// the view's own axes.
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
    /// The offset the view gets this frame.
    applied: Vec2,
    /// The turn the view gets this frame.
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

    /// The offset this shake adds to its camera's view now, along the
    /// view's own axes.
    pub fn applied_offset(&self) -> Vec2 {
        self.state.applied
    }

    /// The turn this shake adds to its camera's view now, in radians.
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

/// Runs every shake `delta` seconds: the offset and turn its view gets
/// this frame. The entity's transform is never written — the view adds
/// them ([`camera::eye`](crate::camera::eye)) — so a tween, a clip or a
/// game setting the transform cannot leave a shake behind in it.
pub(super) fn update(world: &mut World, delta: f32) {
    for (_, shake) in world.query_mut::<CameraShake>() {
        if shake.trauma <= 0.0 {
            shake.state.applied = Vec2::ZERO;
            shake.state.applied_angle = 0.0;
            continue;
        }
        shake.state.time += delta;
        let (offset, angle) = shake.offset();
        shake.trauma = (shake.trauma - shake.decay.max(0.0) * delta).max(0.0);
        shake.state.applied = if offset.is_finite() {
            offset
        } else {
            Vec2::ZERO
        };
        shake.state.applied_angle = if angle.is_finite() { angle } else { 0.0 };
    }
}
