//! Low-pass and high-pass filters: one biquad per side.

use std::f32::consts::PI;

use super::Param;
use crate::ramp::Ramp;

/// How often the coefficients are worked out again while the cutoff or the
/// resonance is ramping, in frames.
///
/// Sixteen frames is a third of a millisecond at 48 kHz: a sweep in steps
/// that short is a sweep, and it costs a `sin` and a `cos` per step rather
/// than per sample.
const RECOMPUTE: u32 = 16;

/// Which side of the cutoff a [`Biquad`] keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    /// Keeps what is below the cutoff.
    Low,
    /// Keeps what is above it.
    High,
}

/// A second-order filter from Robert Bristow-Johnson's *Audio EQ
/// Cookbook*, the formulas every engine's low- and high-pass uses (Web
/// Audio's `BiquadFilterNode` among them), in transposed direct form II.
#[derive(Debug, Clone)]
pub struct Biquad {
    pass: Pass,
    rate: f32,
    cutoff: Ramp,
    q: Ramp,
    /// `b0, b1, b2, a1, a2`, already divided by `a0`.
    coefficients: [f32; 5],
    /// Two state values per side.
    state: [[f32; 2]; 2],
    /// Frames until the coefficients are worked out again while ramping.
    countdown: u32,
}

impl Biquad {
    /// A filter at `cutoff` hertz and resonance `q`, for a device at `rate`.
    pub fn new(pass: Pass, cutoff: f32, q: f32, rate: u32) -> Self {
        let mut filter = Self {
            pass,
            rate: rate.max(1) as f32,
            cutoff: Ramp::new(cutoff),
            q: Ramp::new(q),
            coefficients: [1.0, 0.0, 0.0, 0.0, 0.0],
            state: [[0.0; 2]; 2],
            countdown: 0,
        };
        filter.recompute();
        filter
    }

    /// Ramps `param` to `value` over `frames`, if it is one of this filter's.
    pub fn set(&mut self, param: Param, value: f32, frames: u32) -> bool {
        match param {
            Param::Cutoff => self.cutoff.set(value, frames),
            Param::Q => self.q.set(value, frames),
            _ => return false,
        }
        if frames == 0 {
            self.recompute();
        }
        true
    }

    /// Filters the first two channels of `block` in place.
    pub fn process(&mut self, block: &mut [f32], channels: usize) {
        let sides = channels.min(2);
        for frame in block.chunks_mut(channels) {
            if !(self.cutoff.is_settled() && self.q.is_settled()) {
                if self.countdown == 0 {
                    self.recompute();
                    self.countdown = RECOMPUTE;
                }
                self.countdown -= 1;
                self.cutoff.advance(1);
                self.q.advance(1);
                if self.cutoff.is_settled() && self.q.is_settled() {
                    self.recompute();
                }
            }
            let [b0, b1, b2, a1, a2] = self.coefficients;
            for (sample, state) in frame[..sides].iter_mut().zip(&mut self.state) {
                let x = *sample;
                let y = b0 * x + state[0];
                state[0] = b1 * x - a1 * y + state[1];
                state[1] = b2 * x - a2 * y;
                *sample = y;
            }
        }
    }

    /// The coefficients for where the cutoff and resonance are right now.
    ///
    /// The cutoff is kept between 10 Hz and just under half the rate —
    /// at or past Nyquist the formulas fold back — and the resonance above
    /// a tenth, where `alpha` would blow up.
    fn recompute(&mut self) {
        let cutoff = self.cutoff.value().clamp(10.0, self.rate * 0.49);
        let q = self.q.value().max(0.1);
        let w0 = 2.0 * PI * cutoff / self.rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let (b0, b1, b2) = match self.pass {
            Pass::Low => ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0),
            Pass::High => ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0),
        };
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos;
        let a2 = 1.0 - alpha;
        self.coefficients = [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0];
    }
}
