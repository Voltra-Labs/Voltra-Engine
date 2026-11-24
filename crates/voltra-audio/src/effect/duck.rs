//! A bus pulled down while another one sounds: side-chain ducking.

use super::Param;
use crate::bus::{BusId, MAX_BUSES};

/// A gain that falls to `1 − depth` while the bus it listens to is above
/// its threshold, and rises back once it is not.
///
/// What Unity's Duck Volume effect and Wwise's auto-ducking do, with the
/// same two times: attack to go down, release to come back. Both are
/// straight lines in gain — the duck reaches its depth exactly `attack`
/// after the other bus starts, which is what an author sets it by and what
/// a test can measure. The listened-to bus's level is its voices' peak this
/// block, before its own effects and after its volume (see
/// [`Mixer`](crate::Mixer)), so the order buses are processed in does not
/// matter and nothing lags by a block.
#[derive(Debug, Clone)]
pub struct Duck {
    by: BusId,
    depth: f32,
    threshold: f32,
    /// Frames to go all the way down, and all the way up.
    attack: u32,
    release: u32,
    rate: u32,
    gain: f32,
}

impl Duck {
    /// A duck of `depth` while `by` is above `threshold`, for a device at
    /// `rate`; `attack` and `release` in seconds.
    pub fn new(
        by: BusId,
        depth: f32,
        threshold: f32,
        attack: f32,
        release: f32,
        rate: u32,
    ) -> Self {
        let rate = rate.max(1);
        Self {
            by,
            depth: depth.clamp(0.0, 1.0),
            threshold: threshold.max(0.0),
            attack: seconds_to_frames(attack, rate),
            release: seconds_to_frames(release, rate),
            rate,
            gain: 1.0,
        }
    }

    /// Sets `param` to `value`, if it is one of this duck's.
    ///
    /// Not ramped: the duck's own gain already slides at its attack and
    /// release, so a new depth is reached along those.
    pub fn set(&mut self, param: Param, value: f32) -> bool {
        match param {
            Param::Depth => self.depth = value.clamp(0.0, 1.0),
            Param::Threshold => self.threshold = value.max(0.0),
            Param::Attack => self.attack = seconds_to_frames(value, self.rate),
            Param::Release => self.release = seconds_to_frames(value, self.rate),
            _ => return false,
        }
        true
    }

    /// The gain right now: one when not ducking.
    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// Ducks the first two channels of `block` by what `levels` says the
    /// listened-to bus is doing.
    pub fn process(&mut self, block: &mut [f32], channels: usize, levels: &[f32; MAX_BUSES]) {
        let sounding = levels.get(self.by.index()).copied().unwrap_or(0.0) > self.threshold;
        let target = if sounding { 1.0 - self.depth } else { 1.0 };
        let sides = channels.min(2);
        for frame in block.chunks_mut(channels.max(1)) {
            self.gain = step(self.gain, target, self.depth, self.attack, self.release);
            for sample in &mut frame[..sides] {
                *sample *= self.gain;
            }
        }
    }
}

/// One frame of the gain's slide towards `target`: down by `depth / attack`
/// a frame, up by `depth / release`, and straight there for a time of zero.
fn step(gain: f32, target: f32, depth: f32, attack: u32, release: u32) -> f32 {
    if gain > target {
        if attack == 0 {
            return target;
        }
        (gain - depth.max(f32::EPSILON) / attack as f32).max(target)
    } else if gain < target {
        if release == 0 {
            return target;
        }
        (gain + depth.max(f32::EPSILON) / release as f32).min(target)
    } else {
        gain
    }
}

fn seconds_to_frames(seconds: f32, rate: u32) -> u32 {
    if seconds.is_finite() && seconds > 0.0 {
        (seconds * rate as f32).round() as u32
    } else {
        0
    }
}
