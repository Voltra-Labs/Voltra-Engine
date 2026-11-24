//! A room: Jezar at Dreampoint's Freeverb, the public-domain reverb most
//! engines' "reverb" started as.
//!
//! Eight lowpass-feedback comb filters in parallel per side, summed and
//! smeared by four all-passes in series. The right side's lines are 23
//! samples longer than the left's, which is all the stereo there is. The
//! tunings are Freeverb's own, which are in samples at 44.1 kHz; they are
//! scaled to the device's rate so a room sounds the same size at 48 kHz.

use super::Param;
use crate::ramp::Ramp;

const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
const SPREAD: usize = 23;
const TUNED_AT: f32 = 44_100.0;

/// The input is scaled down by this before the combs, which sum eight
/// copies of it.
const FIXED_GAIN: f32 = 0.015;
/// Room size `0..1` maps to comb feedback `0.7..0.98`.
const SCALE_ROOM: f32 = 0.28;
const OFFSET_ROOM: f32 = 0.7;
/// Damping `0..1` maps to the combs' lowpass coefficient `0..0.4`.
const SCALE_DAMP: f32 = 0.4;
/// The wet signal's make-up gain, Freeverb's `scalewet`.
const SCALE_WET: f32 = 3.0;
/// How far back each all-pass feeds itself.
const ALLPASS_FEEDBACK: f32 = 0.5;

#[derive(Debug, Clone)]
struct Comb {
    line: Box<[f32]>,
    at: usize,
    /// The one-pole lowpass inside the feedback loop: what damping damps.
    store: f32,
}

impl Comb {
    fn new(length: usize) -> Self {
        Self {
            line: vec![0.0; length.max(1)].into_boxed_slice(),
            at: 0,
            store: 0.0,
        }
    }

    fn process(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let output = self.line[self.at];
        self.store = output * (1.0 - damp) + self.store * damp;
        self.line[self.at] = input + self.store * feedback;
        self.at = (self.at + 1) % self.line.len();
        output
    }
}

#[derive(Debug, Clone)]
struct Allpass {
    line: Box<[f32]>,
    at: usize,
}

impl Allpass {
    fn new(length: usize) -> Self {
        Self {
            line: vec![0.0; length.max(1)].into_boxed_slice(),
            at: 0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.line[self.at];
        self.line[self.at] = input + delayed * ALLPASS_FEEDBACK;
        self.at = (self.at + 1) % self.line.len();
        delayed - input
    }
}

/// One side's filters.
#[derive(Debug, Clone)]
struct Side {
    combs: Vec<Comb>,
    allpasses: Vec<Allpass>,
}

impl Side {
    fn new(scale: f32, spread: usize) -> Self {
        let length = |tuning: usize| ((tuning + spread) as f32 * scale).round() as usize;
        Self {
            combs: COMBS.iter().map(|&t| Comb::new(length(t))).collect(),
            allpasses: ALLPASSES.iter().map(|&t| Allpass::new(length(t))).collect(),
        }
    }

    fn process(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let mut sum = 0.0;
        for comb in &mut self.combs {
            sum += comb.process(input, feedback, damp);
        }
        for allpass in &mut self.allpasses {
            sum = allpass.process(sum);
        }
        sum
    }
}

/// Freeverb on the first two channels, every line allocated when it is
/// built on the main thread.
#[derive(Debug, Clone)]
pub struct Reverb {
    left: Side,
    right: Side,
    room_size: Ramp,
    damping: Ramp,
    width: Ramp,
    mix: Ramp,
}

impl Reverb {
    /// A room of `room_size` and `damping`, `width` apart, `mix` wet, for a
    /// device at `rate`.
    pub fn new(room_size: f32, damping: f32, width: f32, mix: f32, rate: u32) -> Self {
        let scale = rate.max(1) as f32 / TUNED_AT;
        Self {
            left: Side::new(scale, 0),
            right: Side::new(scale, SPREAD),
            room_size: Ramp::new(room_size),
            damping: Ramp::new(damping),
            width: Ramp::new(width),
            mix: Ramp::new(mix),
        }
    }

    /// Ramps `param` to `value` over `frames`, if it is one of this room's.
    pub fn set(&mut self, param: Param, value: f32, frames: u32) -> bool {
        match param {
            Param::RoomSize => self.room_size.set(value, frames),
            Param::Damping => self.damping.set(value, frames),
            Param::Width => self.width.set(value, frames),
            Param::Mix => self.mix.set(value, frames),
            _ => return false,
        }
        true
    }

    /// Puts the first two channels of `block` in the room, in place. A mono
    /// device hears the left side.
    pub fn process(&mut self, block: &mut [f32], channels: usize) {
        if channels == 0 {
            return;
        }
        for frame in block.chunks_mut(channels) {
            let feedback = self.room_size.tick().min(1.0) * SCALE_ROOM + OFFSET_ROOM;
            let damp = self.damping.tick().min(1.0) * SCALE_DAMP;
            let width = self.width.tick().min(1.0);
            let mix = self.mix.tick().min(1.0);

            let dry_left = frame[0];
            let dry_right = if channels >= 2 { frame[1] } else { dry_left };
            let input = (dry_left + dry_right) * FIXED_GAIN;

            let wet_left = self.left.process(input, feedback, damp);
            let wet_right = self.right.process(input, feedback, damp);
            let near = SCALE_WET * (width / 2.0 + 0.5);
            let far = SCALE_WET * ((1.0 - width) / 2.0);

            frame[0] = dry_left * (1.0 - mix) + (wet_left * near + wet_right * far) * mix;
            if channels >= 2 {
                frame[1] = dry_right * (1.0 - mix) + (wet_right * near + wet_left * far) * mix;
            }
        }
    }
}
