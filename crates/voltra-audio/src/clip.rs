//! Decoded audio, ready to be mixed.

use std::fmt;
use std::sync::Arc;

/// A whole sound, decoded to interleaved 32-bit float samples.
///
/// Decoded once, at load, and kept resident. Streaming — decoding a long piece
/// of music a block at a time on the audio thread — is a second asset kind
/// with its own buffering, not a flag on this one; Unity draws the same line
/// with `Decompress On Load` versus `Streaming`, and so do Godot and Wwise.
/// Nothing this engine ships yet is long enough to need it.
///
/// Cloning is cheap and is how a clip reaches the audio thread: the samples
/// live behind an `Arc`, so `Clips` keeps the one copy and every voice playing
/// it holds a pointer to the same buffer.
#[derive(Clone)]
pub struct Clip {
    /// Interleaved: frame 0's channels, then frame 1's, and so on. The layout
    /// the device wants, so the mixer never has to gather across planes.
    samples: Arc<[f32]>,
    /// Always at least 1 for a clip built by [`Clip::new`].
    channels: u16,
    /// Samples per second per channel. Always at least 1.
    rate: u32,
}

impl Clip {
    /// A clip over `samples`, interleaved at `channels` and `rate`.
    ///
    /// `channels` and `rate` are floored at 1. Zero in either would divide by
    /// zero in the resampler, and a clip is built either by the decoder —
    /// which rejects both before it gets here — or by a test; neither wants a
    /// `Result` for a value that cannot legitimately be zero. The trailing
    /// partial frame of a buffer whose length is not a multiple of `channels`
    /// is dropped, so `frames * channels` always indexes inside `samples`.
    pub fn new(samples: impl Into<Arc<[f32]>>, channels: u16, rate: u32) -> Self {
        let channels = channels.max(1);
        let rate = rate.max(1);
        let samples = samples.into();

        let whole = samples.len() - samples.len() % usize::from(channels);
        let samples = if whole == samples.len() {
            samples
        } else {
            Arc::from(&samples[..whole])
        };

        Self {
            samples,
            channels,
            rate,
        }
    }

    /// A clip that plays nothing, for a file that would not decode.
    ///
    /// The audible equivalent of the missing-texture checker, except that a
    /// wrong noise is worse than no noise: a placeholder tone would be mixed
    /// into every sound the scene makes, at the volume the author chose for
    /// the real one. Silence plus the warning the store already logs is the
    /// honest answer.
    pub fn silent() -> Self {
        Self::new(Vec::new(), 1, 1)
    }

    /// Interleaved samples, `channels` per frame.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// Channels per frame: 1 for mono, 2 for stereo.
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Samples per second per channel, as the file was recorded.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// How many frames long the clip is.
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels)
    }

    /// How long the clip lasts at its own rate, in seconds.
    pub fn duration(&self) -> f32 {
        self.frames() as f32 / self.rate as f32
    }

    /// Whether there is nothing to play.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// One channel of one frame, or `0.0` past the end.
    ///
    /// Out of range is silence rather than a panic: the resampler steps by a
    /// fractional amount and reads the frame after the one it is between, so
    /// the last frame of every clip asks for a frame that does not exist.
    pub fn sample(&self, frame: usize, channel: u16) -> f32 {
        let channels = usize::from(self.channels);
        let index = frame * channels + usize::from(channel.min(self.channels - 1));
        self.samples.get(index).copied().unwrap_or(0.0)
    }
}

/// Prints the shape, never the samples: a two-second stereo clip is 176 400
/// floats, and `{:?}` on one in a log would scroll a terminal for a minute.
impl fmt::Debug for Clip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Clip")
            .field("frames", &self.frames())
            .field("channels", &self.channels)
            .field("rate", &self.rate)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clip_reports_its_own_shape() {
        let clip = Clip::new(vec![0.1, 0.2, 0.3, 0.4], 2, 44_100);
        assert_eq!(clip.channels(), 2);
        assert_eq!(clip.rate(), 44_100);
        assert_eq!(clip.frames(), 2);
    }

    #[test]
    fn a_duration_is_frames_over_the_clip_s_own_rate() {
        let clip = Clip::new(vec![0.0; 8], 1, 4);
        assert!((clip.duration() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn a_silent_clip_is_empty_but_still_usable() {
        // Divided by in the resampler, so neither of these may be zero even
        // though there is nothing to play.
        let clip = Clip::silent();
        assert!(clip.is_empty());
        assert_eq!(clip.frames(), 0);
        assert_eq!(clip.channels(), 1);
        assert_eq!(clip.rate(), 1);
    }

    #[test]
    fn a_zero_channel_count_is_floored_rather_than_dividing_by_zero() {
        let clip = Clip::new(vec![1.0, 2.0], 0, 0);
        assert_eq!(clip.channels(), 1);
        assert_eq!(clip.rate(), 1);
        assert_eq!(clip.frames(), 2);
    }

    #[test]
    fn a_trailing_partial_frame_is_dropped() {
        // Otherwise `frames * channels` would index past the end for the last
        // frame, and the resampler indexes exactly that way.
        let clip = Clip::new(vec![1.0, 2.0, 3.0], 2, 8);
        assert_eq!(clip.frames(), 1);
        assert_eq!(clip.samples().len(), 2);
    }

    #[test]
    fn a_sample_reads_by_frame_and_channel() {
        let clip = Clip::new(vec![1.0, -1.0, 2.0, -2.0], 2, 8);
        assert_eq!(clip.sample(0, 0), 1.0);
        assert_eq!(clip.sample(0, 1), -1.0);
        assert_eq!(clip.sample(1, 0), 2.0);
        assert_eq!(clip.sample(1, 1), -2.0);
    }

    #[test]
    fn past_the_end_is_silence_rather_than_a_panic() {
        // The frame every voice reads while interpolating its last one.
        let clip = Clip::new(vec![1.0, 2.0], 1, 8);
        assert_eq!(clip.sample(2, 0), 0.0);
        assert_eq!(clip.sample(9_000, 0), 0.0);
    }

    #[test]
    fn a_channel_past_the_last_reads_the_last() {
        // A mono clip asked for its right channel: the mixer asks by index and
        // must not be handed the next frame's first sample instead.
        let clip = Clip::new(vec![1.0, 2.0], 1, 8);
        assert_eq!(clip.sample(0, 1), 1.0);
    }

    #[test]
    fn cloning_shares_the_samples_rather_than_copying_them() {
        let clip = Clip::new(vec![0.0; 1024], 1, 8);
        let copy = clip.clone();
        assert!(std::ptr::eq(
            clip.samples().as_ptr(),
            copy.samples().as_ptr()
        ));
    }

    #[test]
    fn debug_prints_the_shape_not_the_samples() {
        let clip = Clip::new(vec![0.5; 400], 2, 48_000);
        let text = format!("{clip:?}");
        assert!(text.contains("frames: 200"), "got {text}");
        assert!(!text.contains("0.5"), "got {text}");
    }
}
