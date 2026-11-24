//! Probing a file and making the decoder for its audio track.
//!
//! Shared by the whole-file decode and the streaming one: both need the same
//! reader, the same track and the same decoder, and two copies of the
//! choosing would drift.

use std::path::Path;

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::TimeBase;

use crate::error::AudioError;

/// A probed file, ready for its packets to be read and decoded.
pub(crate) struct Opened {
    /// The container, positioned at its first packet.
    pub reader: Box<dyn FormatReader>,
    /// The audio track's decoder.
    pub decoder: Box<dyn AudioDecoder>,
    /// Which track's packets are the audio.
    pub track_id: u32,
    /// What the container says the rate is, before anything decodes.
    pub rate: Option<u32>,
    /// What the container says the channel count is.
    pub channels: Option<usize>,
    /// The unit packet timestamps are in; `None` means one per frame.
    pub time_base: Option<TimeBase>,
    /// How long the track is, in frames, when the container says.
    pub frames: Option<u64>,
}

/// Probes `stream` and opens its default audio track. `path` names it in
/// errors and its extension hints the format.
///
/// The format is taken from the container, not the extension: the extension
/// is passed to `symphonia` only as a hint of where to look first, so a
/// `.ogg` that is really a WAV still decodes.
pub(crate) fn open(stream: MediaSourceStream<'static>, path: &Path) -> Result<Opened, AudioError> {
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(extension);
    }

    let reader = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|source| AudioError::Decode {
            path: path.to_owned(),
            source: Box::new(source),
        })?;

    let track = reader
        .default_track(TrackType::Audio)
        .ok_or_else(|| AudioError::Unsupported {
            path: path.to_owned(),
            reason: "it holds no audio track",
        })?;

    let params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .ok_or_else(|| AudioError::Unsupported {
            path: path.to_owned(),
            reason: "its audio track declares no codec",
        })?;

    let decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|source| AudioError::Decode {
            path: path.to_owned(),
            source: Box::new(source),
        })?;

    let rate = params.sample_rate;
    let channels = params.channels.as_ref().map(|channels| channels.count());
    let track_id = track.id;
    let time_base = track.time_base;
    let frames = track.num_frames;

    Ok(Opened {
        reader,
        decoder,
        track_id,
        rate,
        channels,
        time_base,
        frames,
    })
}

/// Which frame a timestamp in `time_base` units falls on, at `rate`.
///
/// WAV and Ogg Vorbis both count in frames already (a time base of one over
/// the rate), which is the case that costs nothing; the conversion is there
/// for a container that counts in something else.
pub(crate) fn ts_to_frame(ts: i64, time_base: Option<TimeBase>, rate: u32) -> i64 {
    let Some(base) = time_base else {
        return ts;
    };
    let numer = i128::from(base.numer.get());
    let denom = i128::from(base.denom.get());
    if numer == 1 && denom == i128::from(rate) {
        return ts;
    }
    let frame = i128::from(ts) * numer * i128::from(rate) / denom;
    i64::try_from(frame).unwrap_or(if frame < 0 { i64::MIN } else { i64::MAX })
}

/// [`ts_to_frame`] the other way round, for asking a reader to seek.
pub(crate) fn frame_to_ts(frame: i64, time_base: Option<TimeBase>, rate: u32) -> i64 {
    let Some(base) = time_base else {
        return frame;
    };
    let numer = i128::from(base.numer.get());
    let denom = i128::from(base.denom.get());
    if numer == 1 && denom == i128::from(rate) {
        return frame;
    }
    let ts = i128::from(frame) * denom / (numer * i128::from(rate.max(1)));
    i64::try_from(ts).unwrap_or(if ts < 0 { i64::MIN } else { i64::MAX })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_base_of_one_frame_is_the_frame_itself() {
        let base = TimeBase::try_from_recip(48_000);
        assert_eq!(ts_to_frame(1_234, base, 48_000), 1_234);
        assert_eq!(frame_to_ts(1_234, base, 48_000), 1_234);
        assert_eq!(ts_to_frame(77, None, 48_000), 77);
    }

    #[test]
    fn a_time_base_in_milliseconds_is_converted_both_ways() {
        let base = TimeBase::try_new(1, 1_000);
        assert_eq!(ts_to_frame(500, base, 48_000), 24_000);
        assert_eq!(frame_to_ts(24_000, base, 48_000), 500);
    }
}
