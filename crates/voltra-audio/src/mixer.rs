//! Summing every playing voice into one output buffer.
//!
//! The engine's own mixer, not a library's. Unity, Unreal and Godot all write
//! theirs and use the platform only as a device to hand finished buffers to;
//! the Rust ecosystem's playback crates sit exactly where that mixer belongs,
//! which is why this crate takes `cpal` for the device and stops there. It
//! also buys the thing a wrapped mixer cannot give: this type is pure
//! arithmetic with no thread, no device and no clock, so every rule it obeys
//! is a test that runs on a build machine with no sound card.

use std::sync::mpsc::Sender;

use crate::clip::Clip;
use crate::command::Command;
use crate::params::PlayParams;
use crate::voice::{Voice, VoiceId};

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

    /// Carries out one message from the game.
    pub fn apply(&mut self, command: Command) {
        match command {
            Command::Play { id, clip, params } => self.play(id, clip, params),
            Command::SetGain { id, volume, pan } => {
                if let Some(voice) = self.voices.iter_mut().find(|v| v.id() == id) {
                    voice.set_gain(volume, pan);
                }
            }
            // A `Stop` for a voice that already finished is not an error: a
            // game holding an id has no way to know the clip ran out, and the
            // id is never reused, so there is nothing else it could hit.
            Command::Stop(id) => self.remove(|voice| voice.id() == id),
            Command::StopAll => self.remove(|_| true),
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
        for voice in &mut self.voices {
            voice.mix(out, channels);
        }

        // Hard clipping, because a device handed a sample outside the range
        // will do something worse with it than this does — some hosts wrap,
        // which turns a loud moment into a crack. A limiter that ducks the mix
        // instead of squaring it off is a bus effect, and buses are their own
        // stage.
        for sample in out.iter_mut() {
            *sample = sample.clamp(-1.0, 1.0);
        }

        self.remove(|voice| voice.is_finished());
    }

    /// Starts a voice, making room first if the mixer is full.
    fn play(&mut self, id: VoiceId, clip: Clip, params: PlayParams) {
        self.make_room();
        self.voices.push(Voice::new(id, clip, params, self.rate));
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
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    const RATE: u32 = 8;

    fn id(raw: u64) -> VoiceId {
        VoiceId::forge(raw)
    }

    /// A clip of `frames` samples, every one of them `1.0`, at the mixer's own
    /// rate so one output frame is one clip frame.
    fn tone(frames: usize) -> Clip {
        Clip::new(vec![1.0; frames], 1, RATE)
    }

    /// Hard left, so a voice's contribution lands in channel 0 at full gain
    /// and the test reads the arithmetic rather than the pan law.
    fn left() -> PlayParams {
        PlayParams {
            pan: -1.0,
            ..Default::default()
        }
    }

    fn play(mixer: &mut Mixer, raw: u64, frames: usize) -> VoiceId {
        let id = id(raw);
        mixer.apply(Command::Play {
            id,
            clip: tone(frames),
            params: left(),
        });
        id
    }

    fn render(mixer: &mut Mixer, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * usize::from(mixer.channels())];
        mixer.render(&mut out);
        out
    }

    #[test]
    fn a_mixer_with_nothing_playing_renders_silence() {
        let mut mixer = Mixer::new(RATE, 2);
        assert_eq!(render(&mut mixer, 2), vec![0.0; 4]);
    }

    #[test]
    fn a_render_overwrites_whatever_the_device_left_in_the_buffer() {
        // The bug a buffer that is only added into leaves: the previous
        // callback's audio plays again under the silence.
        let mut mixer = Mixer::new(RATE, 2);
        let mut out = vec![0.9; 4];
        mixer.render(&mut out);
        assert_eq!(out, vec![0.0; 4]);
    }

    #[test]
    fn two_voices_sum() {
        let mut mixer = Mixer::new(RATE, 2);
        mixer.apply(Command::Play {
            id: id(1),
            clip: Clip::new(vec![0.25, 0.25], 1, RATE),
            params: left(),
        });
        mixer.apply(Command::Play {
            id: id(2),
            clip: Clip::new(vec![0.5, 0.5], 1, RATE),
            params: left(),
        });

        let out = render(&mut mixer, 2);
        assert!((out[0] - 0.75).abs() < 1e-6, "got {out:?}");
    }

    #[test]
    fn the_sum_is_clipped_rather_than_left_to_the_device() {
        let mut mixer = Mixer::new(RATE, 2);
        for raw in 0..4 {
            play(&mut mixer, raw, 2);
        }
        let out = render(&mut mixer, 2);
        assert!(out.iter().all(|s| *s <= 1.0), "got {out:?}");
        assert!((out[0] - 1.0).abs() < 1e-6, "and reaches the ceiling");
    }

    #[test]
    fn a_voice_that_runs_out_is_dropped() {
        let mut mixer = Mixer::new(RATE, 2);
        play(&mut mixer, 1, 2);
        assert_eq!(mixer.voices(), 1);

        render(&mut mixer, 4);

        assert_eq!(mixer.voices(), 0, "nothing should still be sounding");
    }

    #[test]
    fn stopping_a_voice_takes_it_out_of_the_mix() {
        let mut mixer = Mixer::new(RATE, 2);
        let voice = play(&mut mixer, 1, 64);

        mixer.apply(Command::Stop(voice));

        assert_eq!(mixer.voices(), 0);
        assert_eq!(render(&mut mixer, 2), vec![0.0; 4]);
    }

    #[test]
    fn stopping_a_voice_that_already_finished_is_not_an_error() {
        // A game holding an id cannot know the clip ran out, and ids are never
        // reused, so this must be a no-op rather than hitting something else.
        let mut mixer = Mixer::new(RATE, 2);
        let voice = play(&mut mixer, 1, 1);
        render(&mut mixer, 4);
        let survivor = play(&mut mixer, 2, 64);

        mixer.apply(Command::Stop(voice));

        assert_eq!(mixer.voices(), 1);
        mixer.apply(Command::SetGain {
            id: survivor,
            volume: 1.0,
            pan: -1.0,
        });
    }

    #[test]
    fn stop_all_ends_everything() {
        let mut mixer = Mixer::new(RATE, 2);
        play(&mut mixer, 1, 64);
        play(&mut mixer, 2, 64);

        mixer.apply(Command::StopAll);

        assert_eq!(mixer.voices(), 0);
    }

    #[test]
    fn a_gain_change_reaches_the_voice_it_names() {
        let mut mixer = Mixer::new(RATE, 2);
        let voice = play(&mut mixer, 1, 64);

        mixer.apply(Command::SetGain {
            id: voice,
            volume: 1.0,
            pan: 1.0,
        });

        let out = render(&mut mixer, 1);
        assert!(out[0].abs() < 1e-6, "it should have moved right: {out:?}");
        assert!((out[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_gain_change_for_an_unknown_voice_touches_nothing() {
        let mut mixer = Mixer::new(RATE, 2);
        play(&mut mixer, 1, 64);

        mixer.apply(Command::SetGain {
            id: id(99),
            volume: 0.0,
            pan: 0.0,
        });

        let out = render(&mut mixer, 1);
        assert!((out[0] - 1.0).abs() < 1e-6, "the real voice changed");
    }

    #[test]
    fn a_full_mixer_steals_the_oldest_voice() {
        let mut mixer = Mixer::with_capacity(RATE, 2, 2);
        let oldest = play(&mut mixer, 1, 64);
        play(&mut mixer, 2, 64);

        play(&mut mixer, 3, 64);

        assert_eq!(mixer.voices(), 2, "the ceiling holds");
        mixer.apply(Command::Stop(oldest));
        assert_eq!(mixer.voices(), 2, "and it was the oldest that went");
    }

    #[test]
    fn a_capacity_of_zero_still_plays_one_voice() {
        // A caller that computed a ceiling and got zero must not be handed a
        // mixer that silently drops everything.
        let mut mixer = Mixer::with_capacity(RATE, 2, 0);
        play(&mut mixer, 1, 64);
        assert_eq!(mixer.voices(), 1);
    }

    #[test]
    fn a_finished_voice_hands_its_clip_back_to_be_dropped_elsewhere() {
        // The reason the channel exists: freeing the samples must not happen
        // inside the device callback.
        let (sender, receiver) = channel();
        let mut mixer = Mixer::new(RATE, 2).retiring_to(sender);
        play(&mut mixer, 1, 2);

        render(&mut mixer, 4);

        let returned = receiver.try_recv().expect("the clip should come back");
        assert_eq!(returned.frames(), 2);
    }

    #[test]
    fn a_stolen_voice_hands_its_clip_back_too() {
        let (sender, receiver) = channel();
        let mut mixer = Mixer::with_capacity(RATE, 2, 1).retiring_to(sender);
        play(&mut mixer, 1, 64);
        play(&mut mixer, 2, 64);

        assert!(receiver.try_recv().is_ok());
    }

    #[test]
    fn a_closed_retirement_channel_does_not_stop_the_mix() {
        // The game went away first. The callback has to keep working until the
        // device is torn down.
        let (sender, receiver) = channel();
        let mut mixer = Mixer::new(RATE, 2).retiring_to(sender);
        drop(receiver);
        play(&mut mixer, 1, 1);

        render(&mut mixer, 4);

        assert_eq!(mixer.voices(), 0);
    }

    #[test]
    fn a_degenerate_device_description_is_floored_rather_than_dividing_by_zero() {
        let mixer = Mixer::new(0, 0);
        assert_eq!(mixer.rate(), 1);
        assert_eq!(mixer.channels(), 1);
    }
}
