//! One sound, playing.

use crate::clip::Clip;
use crate::params::PlayParams;

/// A handle to a sound that is playing, or that was.
///
/// Handed out by [`Audio::play`](crate::Audio::play) and used to change or
/// stop that one voice. Never reused: the counter behind it is a `u64`, and a
/// game playing a thousand sounds a second would take half a billion years to
/// wrap. That is what makes a stale id safe to send — the voice it named has
/// finished, and the mixer ignores an id it does not hold rather than stopping
/// whatever took its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VoiceId(u64);

impl VoiceId {
    pub(crate) fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Builds an id without a mixer behind it, for tests that need one to
    /// compare or to send at a voice that is deliberately not there.
    #[doc(hidden)]
    pub fn forge(raw: u64) -> Self {
        Self(raw)
    }
}

/// A clip being read at some rate, at some gain, into the output buffer.
///
/// Lives on the audio thread and is touched from nowhere else: the mixer owns
/// it, and everything the main thread wants to say arrives as a
/// [`Command`](crate::command::Command).
#[derive(Debug)]
pub struct Voice {
    id: VoiceId,
    clip: Clip,
    /// Where in the clip this voice is reading, in frames, fractional.
    ///
    /// `f64` rather than `f32`: a 24-bit mantissa stops being able to
    /// represent every frame index at about 16.7 million, which is six minutes
    /// of 48 kHz audio, and the error before that shows up as drift on a long
    /// loop.
    cursor: f64,
    /// Clip frames per output frame: the resampling ratio and the pitch in one
    /// number, because they multiply.
    step: f64,
    params: PlayParams,
}

impl Voice {
    /// A voice reading `clip` for a device running at `out_rate`.
    pub fn new(id: VoiceId, clip: Clip, params: PlayParams, out_rate: u32) -> Self {
        let step = f64::from(clip.rate()) / f64::from(out_rate.max(1)) * f64::from(params.speed());
        Self {
            id,
            clip,
            cursor: 0.0,
            step,
            params,
        }
    }

    pub fn id(&self) -> VoiceId {
        self.id
    }

    /// Takes the clip out of a voice that is being retired.
    ///
    /// The mixer hands it back to the main thread rather than dropping it
    /// here: this is the last owner of an `Arc` most of the time, and freeing
    /// a megabyte of samples inside the device callback is a pause in the
    /// middle of a buffer nobody can afford.
    pub fn into_clip(self) -> Clip {
        self.clip
    }

    /// Moves the sound without restarting it.
    ///
    /// Volume and pan only: pitch would change [`Self::step`], and a source
    /// whose speed changed every frame as it moved would be a doppler model
    /// nobody asked for.
    pub fn set_gain(&mut self, volume: f32, pan: f32) {
        self.params.volume = volume;
        self.params.pan = pan;
    }

    /// Whether the voice has read past the end of a clip it is not looping.
    pub fn is_finished(&self) -> bool {
        // An empty clip is finished on its first look, looping or not.
        // Otherwise a looping voice on a broken file would spin forever and
        // never free its slot.
        self.clip.is_empty() || (!self.params.looping && self.cursor >= self.clip.frames() as f64)
    }

    /// Adds this voice into `out`, which holds `channels` samples per frame.
    ///
    /// Adds rather than writes: the mixer clears the buffer once and every
    /// voice sums into it, which is what mixing is.
    ///
    /// Channel mapping, and the reason it is this and not a matrix: a mono
    /// clip is panned into the two channels, a stereo one keeps its sides, and
    /// anything wider is read as its first two. Above two output channels the
    /// rest are left alone — surround is a speaker layout with its own
    /// vocabulary, not two channels copied around, and this engine has no way
    /// to author a position in it.
    pub fn mix(&mut self, out: &mut [f32], channels: usize) {
        if channels == 0 || self.clip.is_empty() {
            return;
        }

        let frames = self.clip.frames();
        let (gain_left, gain_right) = self.params.gains();
        let stereo_source = self.clip.channels() >= 2;

        for frame in out.chunks_mut(channels) {
            if !self.params.looping && self.cursor >= frames as f64 {
                return;
            }

            let (left, right) = self.read(frames, stereo_source);
            match channels {
                // One speaker: the two sides are summed at half, so a centred
                // sound is as loud on a mono device as on a stereo one.
                1 => frame[0] += (left * gain_left + right * gain_right) * 0.5,
                _ => {
                    frame[0] += left * gain_left;
                    frame[1] += right * gain_right;
                }
            }

            self.advance(frames);
        }
    }

    /// The two sides of the clip at the current cursor, interpolated.
    ///
    /// Linear interpolation, which is what every mixer uses for a voice that
    /// is not being pitched far: it costs two reads and a multiply, and its
    /// error is inaudible at the ratios a device and a file actually differ
    /// by. A sound pitched down by an octave would want something better, and
    /// that is a filter to choose deliberately rather than a default to pay
    /// for on every voice.
    fn read(&self, frames: usize, stereo_source: bool) -> (f32, f32) {
        let index = self.cursor.floor();
        let fraction = (self.cursor - index) as f32;
        let index = index.max(0.0) as usize;

        // A looping voice interpolates across the seam into frame zero rather
        // than into the silence past the end, or every loop would click.
        let next = if self.params.looping && index + 1 >= frames {
            0
        } else {
            index + 1
        };

        let lerp = |channel: u16| {
            let a = self.clip.sample(index, channel);
            let b = self.clip.sample(next, channel);
            a + (b - a) * fraction
        };

        let left = lerp(0);
        let right = if stereo_source { lerp(1) } else { left };
        (left, right)
    }

    /// Steps the cursor, wrapping a looping voice back into the clip.
    fn advance(&mut self, frames: usize) {
        self.cursor += self.step;

        if !self.params.looping {
            return;
        }
        let length = frames as f64;
        if self.cursor >= length {
            // `%` rather than a subtraction: a voice pitched up far enough to
            // step over the whole clip in one frame would otherwise stay past
            // the end and be silent forever.
            self.cursor %= length;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: VoiceId = VoiceId(1);

    /// A voice over `samples` with the device running at the clip's own rate,
    /// so one output frame is one clip frame and the arithmetic is visible.
    fn voice(samples: Vec<f32>, channels: u16, params: PlayParams) -> Voice {
        Voice::new(ID, Clip::new(samples, channels, 8), params, 8)
    }

    /// Hard left, so a mono voice writes its raw samples into channel 0 and
    /// nothing into channel 1 — the pan law's `1.0` end.
    fn left() -> PlayParams {
        PlayParams {
            pan: -1.0,
            ..Default::default()
        }
    }

    fn mix(voice: &mut Voice, frames: usize, channels: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * channels];
        voice.mix(&mut out, channels);
        out
    }

    #[test]
    fn a_voice_at_the_device_s_own_rate_plays_its_samples_untouched() {
        let mut voice = voice(vec![0.25, 0.5, 0.75], 1, left());
        let out = mix(&mut voice, 3, 2);
        assert_eq!(out, vec![0.25, 0.0, 0.5, 0.0, 0.75, 0.0]);
    }

    #[test]
    fn a_voice_adds_into_the_buffer_rather_than_replacing_it() {
        // What makes a mixer a mixer: two voices in one buffer.
        let mut voice = voice(vec![0.25, 0.25], 1, left());
        let mut out = vec![1.0, 0.0, 1.0, 0.0];
        voice.mix(&mut out, 2);
        assert_eq!(out, vec![1.25, 0.0, 1.25, 0.0]);
    }

    #[test]
    fn a_stereo_clip_keeps_its_sides() {
        let params = PlayParams::default();
        let mut voice = voice(vec![1.0, -1.0, 0.5, -0.5], 2, params);
        let out = mix(&mut voice, 2, 2);

        let (gain_left, gain_right) = params.gains();
        assert!((out[0] - gain_left).abs() < 1e-6);
        assert!((out[1] + gain_right).abs() < 1e-6);
    }

    #[test]
    fn a_mono_clip_reaches_both_sides_when_it_is_centred() {
        let mut voice = voice(vec![1.0], 1, PlayParams::default());
        let out = mix(&mut voice, 1, 2);
        assert!((out[0] - out[1]).abs() < 1e-6);
        assert!(out[0] > 0.0);
    }

    #[test]
    fn a_mono_device_sums_the_two_sides() {
        let mut voice = voice(vec![1.0], 1, PlayParams::default());
        let out = mix(&mut voice, 1, 1);
        // Both sides at 0.707, halved: the same loudness a stereo device gets
        // out of its pair.
        assert!((out[0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    }

    #[test]
    fn a_voice_that_runs_out_leaves_the_rest_of_the_buffer_alone() {
        // The buffer is longer than the sound, which is every callback that
        // contains the end of a one-shot.
        let mut voice = voice(vec![1.0, 1.0], 1, left());
        let out = mix(&mut voice, 4, 2);
        assert_eq!(&out[4..], &[0.0, 0.0, 0.0, 0.0]);
        assert!(voice.is_finished());
    }

    #[test]
    fn a_finished_voice_stays_finished_and_adds_nothing_more() {
        let mut voice = voice(vec![1.0], 1, left());
        mix(&mut voice, 4, 2);
        let again = mix(&mut voice, 4, 2);
        assert_eq!(again, vec![0.0; 8]);
    }

    #[test]
    fn an_empty_clip_is_finished_immediately_even_when_it_loops() {
        // A file that would not decode, played by a source set to loop. The
        // voice must not occupy a slot forever.
        let looping = PlayParams {
            looping: true,
            ..Default::default()
        };
        let mut voice = voice(Vec::new(), 1, looping);
        assert!(voice.is_finished());
        assert_eq!(mix(&mut voice, 4, 2), vec![0.0; 8]);
    }

    #[test]
    fn a_looping_voice_starts_again_at_the_end() {
        let looping = PlayParams {
            looping: true,
            ..left()
        };
        let mut voice = voice(vec![0.5, 0.25], 1, looping);
        let out = mix(&mut voice, 4, 2);
        assert_eq!(out, vec![0.5, 0.0, 0.25, 0.0, 0.5, 0.0, 0.25, 0.0]);
        assert!(!voice.is_finished());
    }

    #[test]
    fn a_looping_voice_pitched_past_the_whole_clip_still_wraps() {
        // The bug a subtraction instead of a modulo leaves behind: one step
        // longer than the clip and the cursor never comes back.
        let looping = PlayParams {
            looping: true,
            pitch: 8.0,
            ..left()
        };
        let mut voice = voice(vec![1.0, 1.0, 1.0, 1.0], 1, looping);
        let out = mix(&mut voice, 4, 2);
        assert!(out.iter().any(|s| *s != 0.0), "it went silent: {out:?}");
        assert!(!voice.is_finished());
    }

    #[test]
    fn a_faster_pitch_reads_further_into_the_clip() {
        let fast = PlayParams {
            pitch: 2.0,
            ..left()
        };
        let mut voice = voice(vec![0.0, 0.1, 0.2, 0.3], 1, fast);
        let out = mix(&mut voice, 2, 2);
        assert!((out[0] - 0.0).abs() < 1e-6);
        assert!((out[2] - 0.2).abs() < 1e-6, "got {out:?}");
    }

    #[test]
    fn a_device_running_faster_than_the_clip_interpolates_between_frames() {
        // The everyday case: a 22 kHz file on a 44 kHz device.
        let clip = Clip::new(vec![0.0, 1.0], 1, 8);
        let mut voice = Voice::new(ID, clip, left(), 16);
        let out = mix(&mut voice, 2, 2);
        assert!((out[0] - 0.0).abs() < 1e-6);
        assert!((out[2] - 0.5).abs() < 1e-6, "got {out:?}");
    }

    #[test]
    fn a_pitch_of_zero_holds_the_first_frame_rather_than_finishing() {
        let held = PlayParams {
            pitch: 0.0,
            ..left()
        };
        let mut voice = voice(vec![0.75, 0.1], 1, held);
        let out = mix(&mut voice, 3, 2);
        assert_eq!(out, vec![0.75, 0.0, 0.75, 0.0, 0.75, 0.0]);
    }

    #[test]
    fn moving_a_voice_changes_its_gains_without_restarting_it() {
        let mut voice = voice(vec![1.0, 1.0, 1.0], 1, left());
        mix(&mut voice, 1, 2);

        voice.set_gain(1.0, 1.0);
        let out = mix(&mut voice, 1, 2);

        assert!(out[0].abs() < 1e-6, "it moved to the other ear");
        assert!((out[1] - 1.0).abs() < 1e-6);
        assert!(!voice.is_finished(), "and did not start over");
    }

    #[test]
    fn a_zero_channel_buffer_is_ignored_rather_than_dividing_by_zero() {
        let mut voice = voice(vec![1.0], 1, left());
        let mut out = Vec::new();
        voice.mix(&mut out, 0);
    }
}
