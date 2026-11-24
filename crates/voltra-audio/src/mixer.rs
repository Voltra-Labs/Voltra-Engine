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
//! The signal path, in order: each voice's own gains and fader, its bus's
//! gain, the sum, the master's gain, and the [`Limiter`]. Every gain on that
//! path is a [`Ramp`], so nothing the game says can step a waveform.

use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::bus::{BusId, MAX_BUSES};
use crate::clip::Clip;
use crate::command::Command;
use crate::limiter::Limiter;
use crate::params::PlayParams;
use crate::ramp::Ramp;
use crate::voice::{frames, Voice, VoiceId};

/// How many voices may sound at once before the oldest is taken for the
/// newest.
///
/// The number is a ceiling on work per buffer, not a musical judgement: 64
/// voices is more than a 2D scene has ever needed at once and still nothing
/// next to the buffer's own cost. `Mixer::with_capacity` is there for the
/// caller that knows better.
pub const DEFAULT_CAPACITY: usize = 64;

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
    /// Where a finished voice's clip is sent to be dropped, when the caller
    /// wired one up. `None` in tests, which drop on this thread and do not
    /// care.
    retired: Option<Sender<Clip>>,
    /// One gain per bus, indexed by [`BusId::index`]; the master's is slot
    /// zero and multiplies the finished sum.
    ///
    /// A fixed array rather than a `Vec` grown as buses are named: the main
    /// thread registers names, and a table the audio thread had to grow
    /// would be an allocation inside the callback. Thirty-two ramps is half a
    /// kilobyte.
    buses: [Ramp; MAX_BUSES],
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
        Self {
            voices: Vec::with_capacity(capacity),
            rate: rate.max(1),
            channels: channels.max(1),
            capacity,
            retired: None,
            buses: [Ramp::new(1.0); MAX_BUSES],
            limiter: Limiter::new(rate.max(1)),
        }
    }

    /// Sends finished clips to `retired` instead of dropping them here.
    ///
    /// See [`Voice::into_clip`]: freeing the samples is the one unbounded
    /// piece of work a callback would otherwise do.
    pub fn retiring_to(mut self, retired: Sender<Clip>) -> Self {
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
                params,
                fade_in,
            } => self.play(id, clip, params, fade_in),
            Command::SetGain { id, volume, pan } => {
                if let Some(voice) = self.voice(id) {
                    voice.set_gain(volume, pan);
                }
            }
            Command::Fade { id, volume, over } => {
                if let Some(voice) = self.voice(id) {
                    voice.fade(volume, over);
                }
            }
            Command::Stop { id, over } => self.stop(|voice| voice.id() == id, over),
            Command::StopAll { over } => self.stop(|_| true, over),
            Command::SetBusVolume { bus, volume, over } => {
                let frames = frames(over, self.rate);
                // An id past the table can only come from a hand-built
                // command; `Buses` never issues one.
                if let Some(ramp) = self.buses.get_mut(bus.index()) {
                    ramp.set(volume, frames);
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
        let frames = u32::try_from(out.len() / channels).unwrap_or(u32::MAX);

        for voice in &mut self.voices {
            // A voice on the master feeds the sum directly: the master's gain
            // is applied once, below, to everything.
            let bus = match voice.bus() {
                BusId::MASTER => Ramp::new(1.0),
                bus => self
                    .buses
                    .get(bus.index())
                    .copied()
                    .unwrap_or(Ramp::new(1.0)),
            };
            voice.mix(out, channels, bus);
        }

        // Every bus advances whether anything was routed into it or not, so a
        // fade on a bus that fell silent still arrives on time.
        for bus in &mut self.buses[1..] {
            bus.advance(frames);
        }

        let master = &mut self.buses[BusId::MASTER.index()];
        for frame in out.chunks_mut(channels) {
            let gain = master.tick();
            for sample in frame {
                *sample *= gain;
            }
        }

        self.limiter.process(out, channels);

        self.remove(|voice| voice.is_finished());
    }

    /// The voice playing under `id`, if it still is.
    fn voice(&mut self, id: VoiceId) -> Option<&mut Voice> {
        self.voices.iter_mut().find(|voice| voice.id() == id)
    }

    /// Starts a voice, making room first if the mixer is full.
    fn play(&mut self, id: VoiceId, clip: Clip, params: PlayParams, fade_in: Duration) {
        self.make_room();
        self.voices
            .push(Voice::new(id, clip, params, self.rate, fade_in));
    }

    /// Ends every voice matching `doomed`, fading it out over `over`.
    ///
    /// A stop shorter than one frame takes the voice out now rather than at
    /// the next render, so a caller that stopped everything sees an empty
    /// mixer straight away.
    fn stop(&mut self, doomed: impl Fn(&Voice) -> bool, over: Duration) {
        if frames(over, self.rate) == 0 {
            self.remove(doomed);
            return;
        }
        for voice in self.voices.iter_mut().filter(|voice| doomed(voice)) {
            voice.stop(over);
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

    /// Hands a finished voice's clip to whoever is dropping them.
    ///
    /// A closed channel is not a failure worth reporting: it means the game
    /// has gone away and this thread is about to as well, and the clip drops
    /// here, which is exactly what would have happened anyway.
    fn retire(&mut self, voice: Voice) {
        let clip = voice.into_clip();
        if let Some(retired) = self.retired.as_ref() {
            let _ = retired.send(clip);
        }
    }
}

#[cfg(test)]
mod tests;
