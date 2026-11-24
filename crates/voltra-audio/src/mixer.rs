//! Summing every playing voice into one output buffer.
//!
//! The engine's own mixer, not a library's. Unity, Unreal and Godot all write
//! theirs and use the platform only as a device to hand finished buffers to;
//! the Rust ecosystem's playback crates sit exactly where that mixer belongs,
//! which is why this crate takes `cpal` for the device and stops there. It
//! also buys the thing a wrapped mixer cannot give: this type is pure
//! arithmetic with no thread, no device and no clock, so every rule it obeys
//! is a test that runs on a build machine with no sound card.
//!
//! The signal path, in order: each voice's own gains and fader, summed into
//! its bus; the bus's effects, first to last, then its gain; every bus summed
//! into the master with the voices played on it directly; the master's
//! effects and gain; and the [`Limiter`]. Every gain on that path is a
//! [`Ramp`], so nothing the game says can step a waveform.

mod render;

use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::bus::{BusId, MAX_BUSES};
use crate::clip::Clip;
use crate::command::Command;
use crate::effect::{Effect, MAX_EFFECTS};
use crate::limiter::Limiter;
use crate::params::PlayParams;
use crate::ramp::{Curve, Ramp};
use crate::retired::Retired;
use crate::stream::Feed;
use crate::voice::{frames, Voice, VoiceId};

/// How many voices may sound at once before the oldest is taken for the
/// newest.
///
/// The number is a ceiling on work per buffer, not a musical judgement: 64
/// voices is more than a 2D scene has ever needed at once and still nothing
/// next to the buffer's own cost. `Mixer::with_capacity` is there for the
/// caller that knows better.
pub const DEFAULT_CAPACITY: usize = 64;

/// How many frames the mixer works through at a time.
///
/// Each bus is summed into a block of its own before its effects run, and
/// the blocks are allocated once, when the mixer is made: a device may hand
/// the callback any length of buffer, and the mixer works through it this
/// much at a time rather than growing anything. 256 frames is 5 ms at
/// 48 kHz; a bus's level, which a duck listens to, is measured per block.
pub const BLOCK: usize = 256;

/// Every voice currently sounding, and the buffer arithmetic that mixes them.
///
/// Owned by the device callback and touched from nowhere else. Everything the
/// game wants to say arrives as a [`Command`] through
/// [`apply`](Self::apply).
#[derive(Debug)]
pub struct Mixer {
    /// Oldest first. The order is load-bearing: it is what
    /// [`Self::make_room`] steals by.
    voices: Vec<Voice>,
    rate: u32,
    channels: u16,
    capacity: usize,
    /// Where a finished voice's clip and feed are sent to be dropped, when
    /// the caller wired one up. `None` in tests, which drop on this thread
    /// and do not care.
    retired: Option<Sender<Retired>>,
    /// One gain per bus, indexed by [`BusId::index`]; the master's is slot
    /// zero and multiplies the finished sum.
    ///
    /// A fixed array rather than a `Vec` grown as buses are named: the main
    /// thread registers names, and a table the audio thread had to grow
    /// would be an allocation inside the callback. Thirty-two ramps is half a
    /// kilobyte.
    buses: [Ramp; MAX_BUSES],
    /// Each bus's effects, in order, by [`BusId::index`].
    chains: [Vec<Effect>; MAX_BUSES],
    /// One [`BLOCK`] of samples per bus, end to end; the master's is first.
    scratch: Vec<f32>,
    /// Each bus's peak level in the block being mixed, for a duck to read.
    levels: [f32; MAX_BUSES],
    limiter: Limiter,
}

impl Mixer {
    /// A mixer for a device running at `rate` with `channels` per frame.
    pub fn new(rate: u32, channels: u16) -> Self {
        Self::with_capacity(rate, channels, DEFAULT_CAPACITY)
    }

    /// As [`new`](Self::new), with a ceiling other than
    /// [`DEFAULT_CAPACITY`].
    pub fn with_capacity(rate: u32, channels: u16, capacity: usize) -> Self {
        let capacity = capacity.max(1);
        let channels = channels.max(1);
        Self {
            voices: Vec::with_capacity(capacity),
            rate: rate.max(1),
            channels,
            capacity,
            retired: None,
            buses: [Ramp::new(1.0); MAX_BUSES],
            chains: std::array::from_fn(|_| Vec::new()),
            scratch: vec![0.0; MAX_BUSES * BLOCK * usize::from(channels)],
            levels: [0.0; MAX_BUSES],
            limiter: Limiter::new(rate.max(1)),
        }
    }

    /// Sends finished voices' clips and feeds, and replaced effect chains, to
    /// `retired` instead of dropping them here.
    ///
    /// See [`Retired`]: freeing the samples is the one unbounded piece of
    /// work a callback would otherwise do.
    pub fn retiring_to(mut self, retired: Sender<Retired>) -> Self {
        self.retired = Some(retired);
        self
    }

    /// Samples per second per channel this mixer renders at.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Samples per frame in the buffers this mixer fills.
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// How many voices are sounding.
    pub fn voices(&self) -> usize {
        self.voices.len()
    }

    /// The gain `bus` is at right now, or `1.0` for an id past the table.
    pub fn bus_volume(&self, bus: BusId) -> f32 {
        self.buses.get(bus.index()).map_or(1.0, Ramp::value)
    }

    /// The effects on `bus`, in order.
    pub fn bus_effects(&self, bus: BusId) -> &[Effect] {
        self.chains.get(bus.index()).map_or(&[], Vec::as_slice)
    }

    /// The peak level `bus` reached in the last block mixed, after its volume
    /// and before its effects: what a duck listening to it hears.
    pub fn bus_level(&self, bus: BusId) -> f32 {
        self.levels.get(bus.index()).copied().unwrap_or(0.0)
    }

    /// Carries out one message from the game.
    ///
    /// Every message naming a voice is ignored for a voice that is not here.
    /// That is not an error: a game holding an id has no way to know the clip
    /// ran out, and the id is never reused, so there is nothing else it could
    /// hit.
    pub fn apply(&mut self, command: Command) {
        match command {
            Command::Play {
                id,
                clip,
                feed,
                params,
                fade_in,
                curve,
            } => self.play(id, clip, feed, params, (fade_in, curve)),
            Command::SetGain { id, volume, pan } => {
                if let Some(voice) = self.voice(id) {
                    voice.set_gain(volume, pan);
                }
            }
            Command::Fade {
                id,
                volume,
                over,
                curve,
            } => {
                if let Some(voice) = self.voice(id) {
                    voice.fade(volume, over, curve);
                }
            }
            Command::Stop { id, over, curve } => self.stop(|voice| voice.id() == id, over, curve),
            Command::StopAll { over } => self.stop(|_| true, over, Curve::Linear),
            Command::SetBusVolume { bus, volume, over } => {
                let frames = frames(over, self.rate);
                // An id past the table can only come from a hand-built
                // command; `Buses` never issues one.
                if let Some(ramp) = self.buses.get_mut(bus.index()) {
                    ramp.set(volume, frames);
                }
            }
            Command::SetBusEffects { bus, mut effects } => {
                let Some(chain) = self.chains.get_mut(bus.index()) else {
                    return;
                };
                // `Audio` caps the chain; this is for a hand-built command.
                effects.truncate(MAX_EFFECTS);
                std::mem::swap(chain, &mut effects);
                self.retire_effects(effects);
            }
            Command::SetBusParam {
                bus,
                effect,
                param,
                value,
                over,
            } => {
                let frames = frames(over, self.rate);
                if let Some(effect) = self
                    .chains
                    .get_mut(bus.index())
                    .and_then(|chain| chain.get_mut(effect))
                {
                    effect.set(param, value, frames);
                }
            }
        }
    }

    /// Fills `out` with everything sounding, `channels()` samples per frame.
    ///
    /// Writes the whole buffer, always. A callback is handed whatever the
    /// device last had in that memory, so a mixer that only added into it
    /// would play the previous buffer again under silence.
    pub fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        let channels = usize::from(self.channels);
        for block in out.chunks_mut(BLOCK * channels) {
            self.render_block(block);
        }
        self.limiter.process(out, channels);
        self.remove(|voice| voice.is_finished());
    }

    /// The voice playing under `id`, if it still is.
    fn voice(&mut self, id: VoiceId) -> Option<&mut Voice> {
        self.voices.iter_mut().find(|voice| voice.id() == id)
    }

    /// Starts a voice, making room first if the mixer is full.
    fn play(
        &mut self,
        id: VoiceId,
        clip: Clip,
        feed: Option<Feed>,
        params: PlayParams,
        (fade_in, curve): (Duration, Curve),
    ) {
        self.make_room();
        self.voices.push(Voice::start(
            id, clip, feed, params, self.rate, fade_in, curve,
        ));
    }

    /// Ends every voice matching `doomed`, fading it out over `over`.
    ///
    /// A stop shorter than one frame takes the voice out now rather than at
    /// the next render, so a caller that stopped everything sees an empty
    /// mixer straight away.
    fn stop(&mut self, doomed: impl Fn(&Voice) -> bool, over: Duration, curve: Curve) {
        if frames(over, self.rate) == 0 {
            self.remove(doomed);
            return;
        }
        for voice in self.voices.iter_mut().filter(|voice| doomed(voice)) {
            voice.stop(over, curve);
        }
    }

    /// Drops the oldest voice if there is no room for another.
    ///
    /// Stealing the oldest rather than refusing the newest: the sound a game
    /// just asked for is the one tied to something that happened on screen,
    /// and the one already half-played is the one a listener will miss least.
    /// Unity calls the same rule voice stealing and defaults to it too.
    fn make_room(&mut self) {
        while self.voices.len() >= self.capacity {
            let stolen = self.voices.remove(0);
            self.retire(stolen);
        }
    }

    /// Takes out every voice matching `doomed`, retiring each one's clip.
    fn remove(&mut self, doomed: impl Fn(&Voice) -> bool) {
        let mut index = 0;
        while index < self.voices.len() {
            if doomed(&self.voices[index]) {
                let voice = self.voices.remove(index);
                self.retire(voice);
            } else {
                index += 1;
            }
        }
    }

    /// Hands a finished voice's clip and feed to whoever is dropping them.
    ///
    /// A closed channel is not a failure worth reporting: it means the game
    /// has gone away and this thread is about to as well, and the clip drops
    /// here, which is exactly what would have happened anyway.
    fn retire(&mut self, voice: Voice) {
        let retired = voice.into_retired();
        if let Some(channel) = self.retired.as_ref() {
            let _ = channel.send(retired);
        }
    }

    /// Hands a replaced chain to whoever is dropping things: its delay
    /// lines are the largest buffers the mixer holds.
    fn retire_effects(&mut self, effects: Vec<Effect>) {
        if effects.is_empty() {
            return;
        }
        if let Some(channel) = self.retired.as_ref() {
            let _ = channel.send(Retired::Effects(effects));
        }
    }
}

#[cfg(test)]
mod tests;
