//! Turning a file on disk into a [`Clip`].
//!
//! `symphonia` does the container and codec work, the way `image` does the PNG
//! work for textures, and it is confined to this file for the same reason: the
//! rest of the crate deals in `f32` samples and should not know what a packet
//! is. Two formats are compiled in — WAV for the short sounds a game triggers,
//! OGG Vorbis for anything long enough that raw PCM would be silly. MP3 is
//! deliberately absent: it buys nothing over Vorbis for a game's own assets.

use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::clip::Clip;
use crate::error::AudioError;

/// Decodes the whole file at `path` into memory.
///
/// The format is taken from the container, not the extension: the extension is
/// passed to `symphonia` only as a hint of where to look first, so a `.ogg`
/// that is really a WAV still decodes. Everything is decoded up front — see
/// [`Clip`] on why there is no streaming path yet.
pub fn decode(path: &Path) -> Result<Clip, AudioError> {
    let file = File::open(path).map_err(|source| AudioError::Read {
        path: path.to_owned(),
        source,
    })?;

    let stream = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(extension);
    }

    let mut reader = symphonia::default::get_probe()
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

    // The decoder is built inside its own scope: `default_track` borrows the
    // reader, and the packet loop below needs it mutably. `declared` is what
    // the container says the track is, kept for the file that holds a valid
    // header and no samples — a truncated recording still has a rate, and a
    // clip that forgot it would resample wrongly if it were ever repaired.
    let (track_id, declared, mut decoder) = {
        let track =
            reader
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

        let declared = (
            params.sample_rate,
            params.channels.as_ref().map(|channels| channels.count()),
        );

        (track.id, declared, decoder)
    };

    let mut samples: Vec<f32> = Vec::new();
    let mut packet_samples: Vec<f32> = Vec::new();
    let mut spec = None;

    loop {
        let packet = match reader.next_packet() {
            Ok(Some(packet)) => packet,
            // The end of the stream, which is how every successful decode
            // finishes.
            Ok(None) => break,
            // A chained OGG: the track list changed mid-file and every decoder
            // would have to be rebuilt. Treating it as the end keeps what was
            // decoded rather than throwing the file away, and a game asset
            // that is a chain of streams is not a case worth carrying code
            // for until one exists.
            Err(SymphoniaError::ResetRequired) => {
                log::warn!(
                    "{} changes format part-way through; keeping what decoded",
                    path.display()
                );
                break;
            }
            Err(source) => {
                return Err(AudioError::Decode {
                    path: path.to_owned(),
                    source: Box::new(source),
                })
            }
        };

        if packet.track_id != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // Both are documented as recoverable: the packet is rubbish and
            // the next one may not be. A file that is entirely rubbish still
            // ends as an empty clip rather than an error, which is the same
            // answer the store would have turned an error into anyway.
            Err(SymphoniaError::IoError(_)) | Err(SymphoniaError::DecodeError(_)) => continue,
            Err(source) => {
                return Err(AudioError::Decode {
                    path: path.to_owned(),
                    source: Box::new(source),
                })
            }
        };

        // Kept as a rate and a channel count rather than the `AudioSpec`
        // itself, which is neither `Copy` nor comparable to what comes next
        // without a clone per packet.
        let this = (decoded.spec().rate(), decoded.spec().channels().count());
        match spec {
            None => spec = Some(this),
            // A rate or channel count that changes part-way cannot be
            // concatenated into one buffer without resampling it here, which
            // is the mixer's job and not the loader's.
            Some(first) if first != this => {
                log::warn!(
                    "{} changes rate or channels part-way through; keeping what decoded",
                    path.display()
                );
                break;
            }
            Some(_) => {}
        }

        // `copy_to_vec_interleaved` resizes its destination, so it fills a
        // scratch buffer that the whole clip is extended from. The scratch
        // keeps its capacity across packets and stops allocating after the
        // first one.
        decoded.copy_to_vec_interleaved(&mut packet_samples);
        samples.extend_from_slice(&packet_samples);
    }

    // What decoded wins; what the container declared is the fallback for a
    // file with a header and no samples.
    let (rate, channels) = match spec {
        Some((rate, channels)) => (rate, channels),
        None => match declared {
            (Some(rate), Some(channels)) => (rate, channels),
            _ => {
                return Err(AudioError::Unsupported {
                    path: path.to_owned(),
                    reason: "nothing in it decoded and it declares no rate or channels",
                })
            }
        },
    };

    let channels = u16::try_from(channels).unwrap_or(u16::MAX);
    if channels == 0 || rate == 0 {
        return Err(AudioError::Unsupported {
            path: path.to_owned(),
            reason: "it declares no channels or no sample rate",
        });
    }

    Ok(Clip::new(samples, channels, rate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_testkit::scratch_root;

    /// A 16-bit PCM WAV of `samples`, written by hand.
    ///
    /// Hand-written rather than encoded by a crate: the point of these tests is
    /// that the decoder reads what a file actually contains, and a fixture
    /// produced by another library would test that library's idea of a WAV
    /// against `symphonia`'s. Forty-four bytes of header is cheaper than a
    /// dev-dependency and says exactly what is on disk.
    fn wav(samples: &[i16], channels: u16, rate: u32) -> Vec<u8> {
        let bytes_per_frame = u32::from(channels) * 2;
        let data_len = samples.len() as u32 * 2;

        let mut file = Vec::new();
        file.extend_from_slice(b"RIFF");
        file.extend_from_slice(&(36 + data_len).to_le_bytes());
        file.extend_from_slice(b"WAVE");

        file.extend_from_slice(b"fmt ");
        file.extend_from_slice(&16u32.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes()); // PCM
        file.extend_from_slice(&channels.to_le_bytes());
        file.extend_from_slice(&rate.to_le_bytes());
        file.extend_from_slice(&(rate * bytes_per_frame).to_le_bytes());
        file.extend_from_slice(&(bytes_per_frame as u16).to_le_bytes());
        file.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

        file.extend_from_slice(b"data");
        file.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            file.extend_from_slice(&sample.to_le_bytes());
        }
        file
    }

    fn write(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = scratch_root().join(name);
        std::fs::write(&path, bytes).expect("the fixture writes");
        path
    }

    #[test]
    fn a_wav_decodes_to_its_own_shape() {
        let path = write("tone.wav", &wav(&[0, 16_384, -16_384, 0], 1, 22_050));

        let clip = decode(&path).expect("a plain PCM WAV decodes");

        assert_eq!(clip.channels(), 1);
        assert_eq!(clip.rate(), 22_050);
        assert_eq!(clip.frames(), 4);
    }

    #[test]
    fn samples_arrive_as_floats_between_minus_one_and_one() {
        let path = write("tone.wav", &wav(&[0, i16::MAX, i16::MIN], 1, 8_000));

        let clip = decode(&path).expect("it decodes");

        assert!(clip.sample(0, 0).abs() < 1e-3);
        assert!(
            (clip.sample(1, 0) - 1.0).abs() < 1e-3,
            "got {}",
            clip.sample(1, 0)
        );
        assert!(
            (clip.sample(2, 0) + 1.0).abs() < 1e-3,
            "got {}",
            clip.sample(2, 0)
        );
    }

    #[test]
    fn a_stereo_wav_keeps_its_channels_interleaved() {
        // Left full, right silent, twice: the layout the mixer indexes by.
        let path = write("stereo.wav", &wav(&[i16::MAX, 0, i16::MAX, 0], 2, 8_000));

        let clip = decode(&path).expect("it decodes");

        assert_eq!(clip.channels(), 2);
        assert_eq!(clip.frames(), 2);
        assert!((clip.sample(0, 0) - 1.0).abs() < 1e-3);
        assert!(clip.sample(0, 1).abs() < 1e-3);
    }

    #[test]
    fn the_container_decides_the_format_not_the_extension() {
        // A WAV someone renamed. The extension is a hint, and a hint that is
        // wrong must not be fatal.
        let path = write("mislabelled.ogg", &wav(&[0, 1_000], 1, 8_000));

        let clip = decode(&path).expect("the hint is only a hint");

        assert_eq!(clip.frames(), 2);
    }

    #[test]
    fn a_file_that_is_not_there_is_a_read_error() {
        let path = scratch_root().join("nowhere.wav");
        assert!(matches!(decode(&path), Err(AudioError::Read { .. })));
    }

    #[test]
    fn a_file_that_is_not_audio_is_a_decode_error_rather_than_a_panic() {
        let path = write("notes.wav", b"this is not a wav at all, not even close");
        assert!(matches!(decode(&path), Err(AudioError::Decode { .. })));
    }

    #[test]
    fn a_wav_with_no_samples_decodes_to_an_empty_clip() {
        // A truncated recording. It has to load: the alternative is a scene
        // that will not open because one sound is zero bytes long.
        let path = write("empty.wav", &wav(&[], 1, 8_000));

        let clip = decode(&path).expect("a header with no data is still a WAV");

        assert!(clip.is_empty());
        assert_eq!(clip.rate(), 8_000, "and still describes its device rate");
    }
}
