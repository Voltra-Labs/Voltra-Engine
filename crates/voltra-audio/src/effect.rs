//! What a bus does to its sound before its volume: filters, echoes, a room,
//! a duck.
//!
//! Inserts in order, the mixing desk's shape and every engine's — Unity's
//! `AudioMixer` group effects, Godot's bus effect slots, Wwise's effect
//! chain. A bus's voices are summed into the bus's own block, the chain runs
//! over it first to last, then the bus's volume, then it joins the master
//! (see [`Mixer`](crate::Mixer)). So a reverb's tail follows the bus's
//! fader, the way a desk's post-insert fader does.
//!
//! **Built on the main thread, run on the audio thread.** An [`EffectDesc`]
//! is the effect as a file or a tick says it; [`Effect::build`] turns it into
//! the running effect at the device's rate, allocating every delay line
//! there, and the mixer is sent the finished chain. Nothing here allocates
//! once built, and a replaced chain goes back to the main thread to be
//! freed ([`Retired`](crate::Retired)).
//!
//! Every effect works on the first two channels, the only ones the mixer
//! writes.

mod biquad;
mod delay;
mod desc;
mod duck;
mod reverb;

use std::time::Duration;

pub use biquad::{Biquad, Pass};
pub use delay::Delay;
pub use desc::{EffectDesc, Param};
pub use duck::Duck;
pub use reverb::Reverb;

use crate::bus::{Buses, MAX_BUSES};

/// The longest a [`Delay`] can be. Its lines are this long whatever its
/// time, so moving the time from a tick never allocates: two seconds of
/// 48 kHz stereo is 768 KB.
pub const MAX_DELAY: Duration = Duration::from_secs(2);

/// How many effects one bus can hold.
///
/// A ceiling on the work a buffer can be asked for, like
/// [`MAX_BUSES`](crate::MAX_BUSES): eight inserts is more than a desk
/// channel shows. A chain longer than this keeps its first eight, with a
/// warning.
pub const MAX_EFFECTS: usize = 8;

/// One running effect on a bus.
#[derive(Debug, Clone)]
pub enum Effect {
    /// A low-pass or high-pass filter.
    Filter(Biquad),
    /// An echo.
    Delay(Delay),
    /// A room.
    Reverb(Box<Reverb>),
    /// Pulled down while another bus sounds.
    Duck(Duck),
}

impl Effect {
    /// The running effect `desc` describes, for a device at `rate`.
    ///
    /// `buses` resolves a duck's `by`, registering the name if nothing has
    /// yet — the same rule as a source naming a bus.
    pub fn build(desc: &EffectDesc, rate: u32, buses: &mut Buses) -> Self {
        match *desc {
            EffectDesc::LowPass { cutoff, q } => {
                Self::Filter(Biquad::new(Pass::Low, cutoff, q, rate))
            }
            EffectDesc::HighPass { cutoff, q } => {
                Self::Filter(Biquad::new(Pass::High, cutoff, q, rate))
            }
            EffectDesc::Delay {
                time,
                feedback,
                mix,
            } => Self::Delay(Delay::new(time, feedback, mix, rate)),
            EffectDesc::Reverb {
                room_size,
                damping,
                width,
                mix,
            } => Self::Reverb(Box::new(Reverb::new(room_size, damping, width, mix, rate))),
            EffectDesc::Duck {
                ref by,
                depth,
                threshold,
                attack,
                release,
            } => Self::Duck(Duck::new(
                buses.id(by),
                depth,
                threshold,
                attack,
                release,
                rate,
            )),
        }
    }

    /// A whole chain, capped at [`MAX_EFFECTS`].
    pub fn build_chain(descs: &[EffectDesc], rate: u32, buses: &mut Buses) -> Vec<Self> {
        if descs.len() > MAX_EFFECTS {
            log::warn!(
                "a bus holds at most {MAX_EFFECTS} effects; keeping the first {MAX_EFFECTS} of {}",
                descs.len()
            );
        }
        descs
            .iter()
            .take(MAX_EFFECTS)
            .map(|desc| Self::build(desc, rate, buses))
            .collect()
    }

    /// Moves `param` to `value` over `frames`: ramped for the filters, the
    /// delay and the room, along the duck's own attack and release for a
    /// duck. `false`, changing nothing, for a parameter this effect does not
    /// have.
    pub fn set(&mut self, param: Param, value: f32, frames: u32) -> bool {
        let value = if value.is_finite() { value } else { 0.0 };
        match self {
            Self::Filter(filter) => filter.set(param, value, frames),
            Self::Delay(delay) => delay.set(param, value, frames),
            Self::Reverb(reverb) => reverb.set(param, value, frames),
            Self::Duck(duck) => duck.set(param, value),
        }
    }

    /// Runs the effect over `block`, `channels` samples a frame, in place.
    /// `levels` is every bus's level this block, which only a duck reads.
    pub fn process(&mut self, block: &mut [f32], channels: usize, levels: &[f32; MAX_BUSES]) {
        if channels == 0 {
            return;
        }
        match self {
            Self::Filter(filter) => filter.process(block, channels),
            Self::Delay(delay) => delay.process(block, channels),
            Self::Reverb(reverb) => reverb.process(block, channels),
            Self::Duck(duck) => duck.process(block, channels, levels),
        }
    }
}

#[cfg(test)]
mod tests;
