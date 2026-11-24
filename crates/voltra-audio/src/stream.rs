//! Long sounds decoded a piece at a time while they play.
//!
//! A resident [`Clip`] is decoded whole at load, which is right for a coin
//! and wrong for four minutes of music: 48 kHz stereo is 23 MB a minute as
//! `f32`. A streamed clip holds only where its bytes are; each voice playing
//! one gets its own decoder, which runs *off* the device callback and feeds
//! the voice through a bounded, lock-free [`ring`] of about a second of
//! samples. Unity's `Streaming` load type, Godot's `AudioStreamOggVorbis`
//! and FMOD's `CREATESTREAM` are the same shape — one decoder per playing
//! instance, a small buffer between it and the mixer.
//!
//! **Who decodes.** Natively a thread of its own ([`Streamer`]), woken every
//! few milliseconds to top every ring up; the callback only ever copies
//! samples out of a ring. A browser build has no threads
//! (`wasm32-unknown-unknown` without atomics), so there the same [`Pump`] is
//! run from the main loop once a frame — the "existing decode path, in
//! pieces": the second of buffer covers a slow frame many times over.
//!
//! **Loop points** belong to the clip (`Clip::with_loop`): a looping voice
//! plays the intro once, then from the loop's start to its end forever. The
//! decoder seeks back itself, so the voice reading the ring never sees the
//! seam.

mod decoder;
mod pump;
mod reader;
pub mod ring;
mod source;
mod streamer;

use std::time::Duration;

pub use decoder::StreamDecoder;
pub use pump::Pump;
pub use reader::Feed;
pub(crate) use reader::StreamReader;
pub use source::{StreamInfo, StreamSource};
pub use streamer::Streamer;

use crate::clip::Clip;
use crate::error::AudioError;

/// How much decoded audio each playing stream keeps queued.
///
/// A second: the native thread tops it up every few milliseconds, so this
/// is slack for a browser's main thread, where the pump runs once a frame
/// and a slow frame — a scene loading — can be a few hundred milliseconds.
/// At 48 kHz stereo it is 384 KB per playing stream, against the 69 MB a
/// three-minute track would be resident.
pub const BUFFER: Duration = Duration::from_secs(1);

/// How much a new stream decodes on the calling thread before its voice
/// starts, so the first buffer the device asks for is already there.
///
/// A tenth of a second: a few milliseconds of decoding on the main thread,
/// once per play, and more than one device buffer on any host.
pub const PRIME: Duration = Duration::from_millis(100);

/// Opens a decoder for a streamed `clip` and the feed a voice reads it from.
///
/// `looping` decides whether the decoder seeks back to the clip's loop
/// start at its loop end, or finishes at the end of the file. The decoder
/// is primed with [`PRIME`] of audio before it is returned.
pub fn start(clip: &Clip, looping: bool) -> Result<(StreamDecoder, Feed), AudioError> {
    let source = clip
        .stream_source()
        .ok_or_else(|| AudioError::Unsupported {
            path: std::path::PathBuf::new(),
            reason: "a resident clip has nothing to stream",
        })?;
    let (mut decoder, feed) = StreamDecoder::new(source.clone(), clip, looping)?;
    decoder.prime();
    Ok((decoder, feed))
}

#[cfg(test)]
mod tests;
