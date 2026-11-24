//! One sound, playing.

use std::time::Duration;

use crate::bus::BusId;
use crate::clip::Clip;
use crate::params::PlayParams;
use crate::ramp::{Curve, Ramp};
use crate::retired::Retired;
use crate::stream::{Feed, StreamReader};

/// How long a gain change takes to arrive when nobody asked for a fade.
///
/// Fifteen milliseconds: long enough that a source moving across the screen,
/// re-panned every frame, never steps inside a buffer, and short enough that
/// nobody hears it lag. The same number for a de-clicked stop, which is the
/// same problem — a waveform cut off mid-cycle is a step.
pub const SMOOTHING: Duration = Duration::from_millis(15);

/// How many output frames `time` lasts at `rate`, rounded to the nearest.
///
/// Saturating: a fade longer than `u32::MAX` frames is a day at 48 kHz, and
/// arriving after a day instead of after a week is not a bug anyone files.
pub fn frames(time: Duration, rate: u32) -> u32 {
    let frames = (time.as_secs_f64() * f64::from(rate)).round();
    if frames >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        frames as u32
    }
}

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
    /// Unused for a streamed clip, whose reader keeps its own place.
    ///
    /// `f64` rather than `f32`: a 24-bit mantissa stops being able to
    /// represent every frame index at about 16.7 million, which is six minutes
    /// of 48 kHz audio, and the error before that shows up as drift on a long
    /// loop.
    cursor: f64,
    /// Clip frames per output frame: the resampling ratio and the pitch in one
    /// number, because they multiply.
    step: f64,
    looping: bool,
    bus: BusId,
    /// The two channel gains volume and pan describe, smoothed.
    ///
    /// Ramped as gains rather than as a volume and a pan because a pan runs
    /// negative and a [`Ramp`] does not — and because the gains are what the
    /// multiply needs, so ramping them costs nothing at the point of use.
    left: Ramp,
    right: Ramp,
    /// The tween on top: fades in, fades out, cross-fades.
    ///
    /// Separate from `left` and `right` because those are rewritten every
    /// frame by whatever positions the source, and a fade that the next
    /// frame's position undid would not be a fade.
    fader: Ramp,
    /// Whether the fader is taking this voice out. Once it arrives the voice
    /// is finished, wherever its cursor is.
    stopping: bool,
    /// [`SMOOTHING`] in this device's frames.
    smoothing: u32,
    /// The device's rate, kept to convert the fades that arrive later.
    out_rate: u32,
    /// Where a streamed clip's frames come from; `None` for a resident one.
    stream: Option<StreamReader>,
}

impl Voice {
    /// A voice reading `clip` for a device running at `out_rate`, rising from
    /// silence over `fade_in` in a straight line.
    pub fn new(
        id: VoiceId,
        clip: Clip,
        params: PlayParams,
        out_rate: u32,
        fade_in: Duration,
    ) -> Self {
        Self::start(id, clip, None, params, out_rate, fade_in, Curve::Linear)
    }

    /// The general form of [`new`](Self::new): `feed` is the stream a
    /// streamed clip plays from, and `curve` the shape of the fade in.
    ///
    /// A streamed clip with no feed plays nothing and is finished at once:
    /// the stream could not be opened, and the main thread already said why.
    pub fn start(
        id: VoiceId,
        clip: Clip,
        feed: Option<Feed>,
        params: PlayParams,
        out_rate: u32,
        fade_in: Duration,
        curve: Curve,
    ) -> Self {
        let out_rate = out_rate.max(1);
        let step = f64::from(clip.rate()) / f64::from(out_rate) * f64::from(params.speed());
        let (left, right) = params.gains();

        let mut fader = Ramp::new(1.0);
        let fade_in = frames(fade_in, out_rate);
        if fade_in > 0 {
            fader.jump(0.0);
            fader.set_curved(1.0, fade_in, curve);
        }

        Self {
            id,
            clip,
            cursor: 0.0,
            step,
            looping: params.looping,
            bus: params.bus,
            // The opening gain is a jump: there is no previous level for a
            // ramp to come from, and a voice that faded up from silence on
            // every start would soften every one-shot's attack.
            left: Ramp::new(left),
            right: Ramp::new(right),
            fader,
            stopping: false,
            smoothing: frames(SMOOTHING, out_rate),
            out_rate,
            stream: feed.map(StreamReader::new),
        }
    }

    /// The id the voice was started under.
    pub fn id(&self) -> VoiceId {
        self.id
    }

    /// The bus this voice is heard on.
    pub fn bus(&self) -> BusId {
        self.bus
    }

    /// Takes apart a voice that is being retired — see [`Retired`].
    pub fn into_retired(self) -> Retired {
        Retired::Voice {
            clip: self.clip,
            feed: self.stream.map(StreamReader::into_feed),
        }
    }

    /// Moves the sound without restarting it, arriving over [`SMOOTHING`].
    ///
    /// Volume and pan only: pitch would change [`Self::step`], and a source
    /// whose speed changed every frame as it moved would be a doppler model
    /// nobody asked for.
    pub fn set_gain(&mut self, volume: f32, pan: f32) {
        let (left, right) = PlayParams {
            volume,
            pan,
            ..Default::default()
        }
        .gains();
        self.left.set(left, self.smoothing);
        self.right.set(right, self.smoothing);
    }

    /// Slides the fader to `volume` over `over`, along `curve`.
    ///
    /// Cancels a stop in progress: a fade-out interrupted by a fade-in is a
    /// voice the game has changed its mind about, and the ramp carries on from
    /// the level actually being heard.
    pub fn fade(&mut self, volume: f32, over: Duration, curve: Curve) {
        self.stopping = false;
        self.fader
            .set_curved(volume, frames(over, self.out_rate), curve);
    }

    /// Fades the voice to silence over `over` along `curve`, and ends it
    /// there.
    pub fn stop(&mut self, over: Duration, curve: Curve) {
        self.stopping = true;
        self.fader
            .set_curved(0.0, frames(over, self.out_rate), curve);
    }

    /// Whether the voice has read past the end of a clip it is not looping,
    /// or has finished fading out.
    pub fn is_finished(&self) -> bool {
        if self.stopping && self.fader.is_settled() {
            return true;
        }
        if self.clip.is_streamed() {
            return self.stream.as_ref().is_none_or(StreamReader::is_finished);
        }
        // An empty clip is finished on its first look, looping or not.
        // Otherwise a looping voice on a broken file would spin forever and
        // never free its slot.
        self.clip.is_empty() || (!self.looping && self.cursor >= self.clip.frames() as f64)
    }

    /// Adds this voice into `out`, which holds `channels` samples per frame.
    ///
    /// Adds rather than writes: the mixer clears the buffer once and every
    /// voice sums into it, which is what mixing is.
    ///
    /// `bus` is a copy of the bus's ramp as the buffer starts. Every voice on
    /// the bus steps its own copy frame by frame, so all of them hear the same
    /// fade at the same frame, and the mixer advances the real one once
    /// afterwards. That is per-bus gain without a per-bus buffer to sum into.
    ///
    /// Channel mapping, and the reason it is this and not a matrix: a mono
    /// clip is panned into the two channels, a stereo one keeps its sides, and
    /// anything wider is read as its first two. Above two output channels the
    /// rest are left alone — surround is a speaker layout with its own
    /// vocabulary, not two channels copied around, and this engine has no way
    /// to author a position in it.
    pub fn mix(&mut self, out: &mut [f32], channels: usize, mut bus: Ramp) {
        if channels == 0 || self.is_finished() {
            return;
        }

        let frames = self.clip.frames();
        let stereo_source = self.clip.channels() >= 2;

        for frame in out.chunks_mut(channels) {
            if self.is_finished() {
                return;
            }

            let level = self.fader.tick() * bus.tick();
            let gain_left = self.left.tick() * level;
            let gain_right = self.right.tick() * level;
            let (left, right) = match &self.stream {
                Some(stream) => stream.read(),
                None => self.read(frames, stereo_source),
            };
            match channels {
                // One speaker: the two sides are summed at half, so a centred
                // sound is as loud on a mono device as on a stereo one.
                1 => frame[0] += (left * gain_left + right * gain_right) * 0.5,
                _ => {
                    frame[0] += left * gain_left;
                    frame[1] += right * gain_right;
                }
            }

            match &mut self.stream {
                Some(stream) => stream.advance(self.step),
                None => self.advance(frames),
            }
        }
    }

    /// The frame a looping voice turns back at: the loop's end, or the
    /// clip's.
    fn loop_end(&self, frames: usize) -> f64 {
        match self.clip.loop_end() {
            Some(end) => end as f64,
            None => frames as f64,
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

        // A looping voice interpolates across the seam into the loop's start
        // rather than into the silence past its end, or every loop would
        // click.
        let next = if self.looping && (index + 1) as f64 >= self.loop_end(frames) {
            self.clip.loop_start() as usize
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

    /// Steps the cursor, wrapping a looping voice back to its loop's start.
    fn advance(&mut self, frames: usize) {
        self.cursor += self.step;

        if !self.looping {
            return;
        }
        let end = self.loop_end(frames);
        if self.cursor >= end {
            // `%` rather than a subtraction: a voice pitched up far enough to
            // step over the whole loop in one frame would otherwise stay past
            // the end and be silent forever.
            let start = self.clip.loop_start() as f64;
            let length = end - start;
            self.cursor = start + (self.cursor - start) % length;
        }
    }
}

#[cfg(test)]
mod tests;
