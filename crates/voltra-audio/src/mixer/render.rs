//! Mixing one block: voices into their buses, each bus through its effects
//! and its gain into the master, the master through its own.

use super::{Mixer, BLOCK};
use crate::bus::{BusId, MAX_BUSES};

impl Mixer {
    /// Mixes up to [`BLOCK`] frames into `block`, which the caller has
    /// zeroed.
    ///
    /// A bus is worked on only if a voice is playing on it or it has
    /// effects — a delay's echoes and a reverb's tail go on after the last
    /// voice has stopped, so a bus with effects is always run. Every other
    /// bus only has its gain advanced, so a fade on a bus that fell silent
    /// still arrives on time.
    ///
    /// Levels are measured after every voice is summed and before any chain
    /// runs, so a duck hears the bus it listens to in this very block,
    /// whichever order the two buses are in.
    pub(super) fn render_block(&mut self, block: &mut [f32]) {
        let channels = usize::from(self.channels);
        let frames = block.len() / channels;
        let used = frames * channels;
        let stride = BLOCK * channels;
        let master = BusId::MASTER.index();

        let mut occupied = [false; MAX_BUSES];
        occupied[master] = true;
        for voice in &self.voices {
            occupied[voice.bus().index().min(MAX_BUSES - 1)] = true;
        }
        let mut active = [false; MAX_BUSES];
        for (bus, active) in active.iter_mut().enumerate() {
            *active = occupied[bus] || !self.chains[bus].is_empty();
            if *active {
                self.scratch[bus * stride..bus * stride + used].fill(0.0);
            }
        }

        for voice in &mut self.voices {
            let bus = voice.bus().index().min(MAX_BUSES - 1);
            let start = bus * stride;
            voice.mix(&mut self.scratch[start..start + used], channels);
        }

        for (bus, level) in self.levels.iter_mut().enumerate() {
            *level = if occupied[bus] {
                let start = bus * stride;
                peak(&self.scratch[start..start + used]) * self.buses[bus].value()
            } else {
                0.0
            };
        }

        let (sum, rest) = self.scratch.split_at_mut(stride);
        let sum = &mut sum[..used];
        for (bus, &on) in active.iter().enumerate().skip(1) {
            if !on {
                self.buses[bus].advance(u32::try_from(frames).unwrap_or(u32::MAX));
                continue;
            }
            let start = (bus - 1) * stride;
            let samples = &mut rest[start..start + used];
            for effect in &mut self.chains[bus] {
                effect.process(samples, channels, &self.levels);
            }
            let gain = &mut self.buses[bus];
            for (into, from) in sum.chunks_mut(channels).zip(samples.chunks(channels)) {
                let level = gain.tick();
                for (into, from) in into.iter_mut().zip(from) {
                    *into += from * level;
                }
            }
        }

        for effect in &mut self.chains[master] {
            effect.process(sum, channels, &self.levels);
        }
        let gain = &mut self.buses[master];
        for (out, from) in block.chunks_mut(channels).zip(sum.chunks(channels)) {
            let level = gain.tick();
            for (out, from) in out.iter_mut().zip(from) {
                *out = from * level;
            }
        }
    }
}

/// The largest magnitude in `samples`.
fn peak(samples: &[f32]) -> f32 {
    samples
        .iter()
        .fold(0.0, |peak, sample| peak.max(sample.abs()))
}
