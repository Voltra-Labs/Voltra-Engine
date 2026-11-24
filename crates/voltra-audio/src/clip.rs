//! Decoded audio, ready to be mixed — or where to decode it from.

use std::fmt;
use std::sync::Arc;

use crate::stream::{StreamInfo, StreamSource};

/// A sound: decoded whole and resident, or [streamed](crate::stream) from
/// its file while it plays.
///
/// One type with two representations, which is Unity's line: an `AudioClip`
/// is either `Decompress On Load` or `Streaming`, chosen per asset in its
/// import settings, and everything that plays a clip plays either. Here the
/// choice is the asset's too — a `*.clip.ron` saying `stream: true` (see
/// `voltra_assets::Clips`) — and a voice is handed the samples or a feed of
/// them depending on which this is.
///
/// Cloning is cheap and is how a clip reaches the audio thread: the samples
/// (or the stream's source) live behind an `Arc`, so `Clips` keeps the one
/// copy and every voice playing it holds a pointer to the same buffer.
#[derive(Clone)]
pub struct Clip {
    /// Interleaved: frame 0's channels, then frame 1's, and so on. The layout
    /// the device wants, so the mixer never has to gather across planes.
    /// Empty for a streamed clip.
    samples: Arc<[f32]>,
    /// Always at least 1 for a clip built by [`Clip::new`].
    channels: u16,
    /// Samples per second per channel. Always at least 1.
    rate: u32,
    /// Where a looping voice goes back to, in frames.
    loop_start: u64,
    /// Where a looping voice turns back, in frames; `None` is the end.
    loop_end: Option<u64>,
    /// Where a streamed clip's bytes are; `None` for a resident one.
    stream: Option<Arc<StreamSource>>,
    /// How long a streamed clip's header says it is.
    declared_frames: Option<u64>,
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
            loop_start: 0,
            loop_end: None,
            stream: None,
            declared_frames: None,
        }
    }

    /// A clip decoded from `source` while it plays, shaped as `info` says.
    pub fn streamed(source: Arc<StreamSource>, info: StreamInfo) -> Self {
        Self {
            samples: Arc::from(Vec::new()),
            channels: info.channels.max(1),
            rate: info.rate.max(1),
            loop_start: 0,
            loop_end: None,
            stream: Some(source),
            declared_frames: info.frames,
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

    /// The same clip, looping between `start` and `end` frames when a voice
    /// plays it looping: the intro once, then the loop forever.
    ///
    /// `end` of `None`, or past the clip's end, is the end. A region that is
    /// empty or backwards is no region — the whole clip loops — because a
    /// loop of nothing would be a voice stuck on one frame.
    pub fn with_loop(mut self, start: u64, end: Option<u64>) -> Self {
        let length = self.known_frames();
        let end = match (end, length) {
            (Some(end), Some(length)) if end >= length => None,
            (end, _) => end,
        };
        let limit = end.or(length);
        if limit.is_some_and(|limit| start >= limit) {
            self.loop_start = 0;
            self.loop_end = None;
        } else {
            self.loop_start = start;
            self.loop_end = end;
        }
        self
    }

    /// Where a looping voice goes back to, in frames.
    pub fn loop_start(&self) -> u64 {
        self.loop_start
    }

    /// Where a looping voice turns back, in frames; `None` is the end.
    pub fn loop_end(&self) -> Option<u64> {
        self.loop_end
    }

    /// Whether this clip is decoded while it plays rather than resident.
    pub fn is_streamed(&self) -> bool {
        self.stream.is_some()
    }

    /// Where a streamed clip's bytes are.
    pub fn stream_source(&self) -> Option<&Arc<StreamSource>> {
        self.stream.as_ref()
    }

    /// Whether `other` is this same clip — the same samples or the same
    /// stream — rather than an equal one.
    pub fn same(&self, other: &Clip) -> bool {
        match (&self.stream, &other.stream) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => Arc::ptr_eq(&self.samples, &other.samples),
            _ => false,
        }
    }

    /// Interleaved samples, `channels` per frame. Empty for a streamed clip.
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

    /// How many frames long the clip is; for a streamed one, what its header
    /// declared, or `0` when it declared nothing.
    pub fn frames(&self) -> usize {
        match self.known_frames() {
            Some(frames) => usize::try_from(frames).unwrap_or(usize::MAX),
            None => 0,
        }
    }

    /// The length when it is known: always for a resident clip.
    fn known_frames(&self) -> Option<u64> {
        if self.stream.is_some() {
            self.declared_frames
        } else {
            Some((self.samples.len() / usize::from(self.channels)) as u64)
        }
    }

    /// How long the clip lasts at its own rate, in seconds.
    pub fn duration(&self) -> f32 {
        self.frames() as f32 / self.rate as f32
    }

    /// Whether there is nothing to play. A streamed clip is empty only when
    /// its header says it is zero frames long.
    pub fn is_empty(&self) -> bool {
        match self.stream {
            Some(_) => self.declared_frames == Some(0),
            None => self.samples.is_empty(),
        }
    }

    /// One channel of one frame, or `0.0` past the end — and always for a
    /// streamed clip, whose samples are not here.
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
            .field("streamed", &self.is_streamed())
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

    #[test]
    fn a_loop_region_is_kept_when_it_fits() {
        let clip = Clip::new(vec![0.0; 10], 1, 8).with_loop(2, Some(6));
        assert_eq!((clip.loop_start(), clip.loop_end()), (2, Some(6)));
    }

    #[test]
    fn a_loop_end_past_the_clip_is_the_end() {
        let clip = Clip::new(vec![0.0; 10], 1, 8).with_loop(2, Some(60));
        assert_eq!((clip.loop_start(), clip.loop_end()), (2, None));
    }

    #[test]
    fn a_backwards_or_empty_loop_is_the_whole_clip() {
        // A loop of nothing would hold a voice on one frame forever.
        let backwards = Clip::new(vec![0.0; 10], 1, 8).with_loop(6, Some(2));
        assert_eq!((backwards.loop_start(), backwards.loop_end()), (0, None));
        let past = Clip::new(vec![0.0; 10], 1, 8).with_loop(12, None);
        assert_eq!((past.loop_start(), past.loop_end()), (0, None));
    }

    #[test]
    fn a_clip_is_the_same_as_its_clone_and_not_as_an_equal_one() {
        let clip = Clip::new(vec![0.5; 4], 1, 8);
        assert!(clip.same(&clip.clone()));
        assert!(!clip.same(&Clip::new(vec![0.5; 4], 1, 8)));
    }

    #[test]
    fn a_streamed_clip_reports_its_header_and_holds_no_samples() {
        let source = Arc::new(StreamSource::File("music.ogg".into()));
        let clip = Clip::streamed(
            source,
            StreamInfo {
                channels: 2,
                rate: 44_100,
                frames: Some(441_000),
            },
        );
        assert!(clip.is_streamed());
        assert!(!clip.is_empty());
        assert!(clip.samples().is_empty());
        assert_eq!(clip.frames(), 441_000);
        assert!((clip.duration() - 10.0).abs() < 1e-4);
    }
}
