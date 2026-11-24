//! An echo: a delay line per side, read a fractional distance behind where
//! it is written, fed back into itself.

use super::{Param, MAX_DELAY};
use crate::ramp::Ramp;

/// The most feedback allowed: below one, so every echo is quieter than the
/// one before and the tail always dies.
const MAX_FEEDBACK: f32 = 0.95;

/// A feedback delay, its lines allocated once, at [`MAX_DELAY`], when it is
/// built on the main thread — so moving the time from a tick never
/// allocates on the audio thread.
#[derive(Debug, Clone)]
pub struct Delay {
    /// One line per side, each `length` samples.
    lines: [Box<[f32]>; 2],
    length: usize,
    /// Where the next sample is written.
    write: usize,
    rate: f32,
    time: Ramp,
    feedback: Ramp,
    mix: Ramp,
}

impl Delay {
    /// A delay of `time` seconds for a device at `rate`.
    pub fn new(time: f32, feedback: f32, mix: f32, rate: u32) -> Self {
        let rate = rate.max(1);
        // Two more than the longest delay: the read interpolates between two
        // samples, and the newest is the one being written.
        let length = (MAX_DELAY.as_secs_f32() * rate as f32).ceil() as usize + 2;
        Self {
            lines: [
                vec![0.0; length].into_boxed_slice(),
                vec![0.0; length].into_boxed_slice(),
            ],
            length,
            write: 0,
            rate: rate as f32,
            time: Ramp::new(time),
            feedback: Ramp::new(feedback),
            mix: Ramp::new(mix),
        }
    }

    /// Ramps `param` to `value` over `frames`, if it is one of this delay's.
    ///
    /// A time that ramps reads the line at a moving distance, which bends
    /// the pitch of what is in it the way tape does — the honest sound of a
    /// delay being moved, and no click.
    pub fn set(&mut self, param: Param, value: f32, frames: u32) -> bool {
        match param {
            Param::Time => self.time.set(value, frames),
            Param::Feedback => self.feedback.set(value, frames),
            Param::Mix => self.mix.set(value, frames),
            _ => return false,
        }
        true
    }

    /// Echoes the first two channels of `block` in place.
    pub fn process(&mut self, block: &mut [f32], channels: usize) {
        let sides = channels.min(2);
        let longest = (self.length - 2) as f32;
        for frame in block.chunks_mut(channels) {
            let delay = (self.time.tick() * self.rate).clamp(1.0, longest);
            let feedback = self.feedback.tick().min(MAX_FEEDBACK);
            let mix = self.mix.tick().min(1.0);

            let whole = delay.floor();
            let fraction = delay - whole;
            let behind = whole as usize;
            let newer = (self.write + self.length - behind) % self.length;
            let older = (newer + self.length - 1) % self.length;

            for (sample, line) in frame[..sides].iter_mut().zip(&mut self.lines) {
                let echo = line[newer] + (line[older] - line[newer]) * fraction;
                let dry = *sample;
                line[self.write] = dry + echo * feedback;
                *sample = dry * (1.0 - mix) + echo * mix;
            }
            self.write = (self.write + 1) % self.length;
        }
    }
}
