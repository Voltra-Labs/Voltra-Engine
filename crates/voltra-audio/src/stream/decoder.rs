//! One playing stream's decoder: packets in, samples into its ring.

use std::path::PathBuf;
use std::sync::Arc;

use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{SeekMode, SeekTo};
use symphonia::core::units::Timestamp;

use super::reader::Feed;
use super::ring::{ring, Producer};
use super::source::StreamSource;
use super::{BUFFER, PRIME};
use crate::clip::Clip;
use crate::decode::{frame_to_ts, open, ts_to_frame, Opened};
use crate::error::AudioError;

/// How far before a loop's start the decoder seeks, so a codec that needs a
/// packet of history (Vorbis overlaps each block with the last) has it, and
/// the frames before the loop are thrown away by timestamp.
const PREROLL: u64 = 4_096;

/// What [`StreamDecoder::fill`] left the stream as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// More to decode; the ring is full for now.
    Running,
    /// Nothing more will be decoded: the end of a file that does not loop,
    /// a file that stopped decoding, or a voice that went away.
    Done,
}

/// Decodes one voice's stream into the ring that voice reads.
///
/// Lives wherever the decoding runs — the [`Streamer`](super::Streamer)
/// thread natively, the main thread on the web — and never on the audio
/// thread. Its buffers grow to the largest packet once and are reused.
pub struct StreamDecoder {
    source: Arc<StreamSource>,
    path: PathBuf,
    opened: Opened,
    producer: Producer,
    /// The rate and channel count the voice was told, which every packet has
    /// to match.
    rate: u32,
    channels: usize,
    /// How many channels are queued per frame: the first two at most, since
    /// those are all the mixer reads.
    queued: usize,
    looping: bool,
    loop_start: u64,
    loop_end: Option<u64>,
    /// Frames before this one are decoded and thrown away: zero from the
    /// start, the loop's start after seeking back.
    skip_until: i64,
    /// The last packet's samples, interleaved as decoded.
    decoded: Vec<f32>,
    /// The part of the last packet waiting for room in the ring, already cut
    /// down to `queued` channels and to the frames that are wanted.
    pending: Vec<f32>,
    pending_at: usize,
    /// Whether, once `pending` is in the ring, the decoder turns back to the
    /// loop's start.
    wrap_after_pending: bool,
    /// Times the decoder has turned back since it last decoded a wanted
    /// frame. A loop that keeps turning back with nothing in it — a header
    /// that promised frames the file does not hold — would spin the pump
    /// forever without this.
    empty_laps: u8,
    done: bool,
}

impl StreamDecoder {
    /// A decoder for `clip` from `source`, and the feed its voice reads.
    pub fn new(
        source: Arc<StreamSource>,
        clip: &Clip,
        looping: bool,
    ) -> Result<(Self, Feed), AudioError> {
        let path = source.path().to_owned();
        let opened = open(source.open()?, &path)?;
        let rate = clip.rate();
        let channels = usize::from(clip.channels());
        let queued = channels.min(2);

        let frames = (BUFFER.as_secs_f64() * f64::from(rate)).ceil() as usize;
        let (producer, consumer) = ring(frames.max(1) * queued);

        let decoder = Self {
            source,
            path,
            opened,
            producer,
            rate,
            channels,
            queued,
            looping,
            loop_start: clip.loop_start(),
            loop_end: clip.loop_end(),
            skip_until: 0,
            decoded: Vec::new(),
            pending: Vec::new(),
            pending_at: 0,
            wrap_after_pending: false,
            empty_laps: 0,
            done: false,
        };
        Ok((decoder, Feed::new(consumer, queued as u16)))
    }

    /// Decodes [`PRIME`] of audio now, so the voice has something to play in
    /// its first buffer.
    pub fn prime(&mut self) {
        let frames = (PRIME.as_secs_f64() * f64::from(self.rate)).ceil() as usize;
        let target = (frames * self.queued).min(self.producer.capacity());
        while self.producer.capacity() - self.producer.space() < target {
            if self.step() == Progress::Done || self.is_full() {
                break;
            }
        }
    }

    /// Tops the ring up, decoding as many packets as fit.
    pub fn fill(&mut self) -> Progress {
        if self.producer.is_closed() {
            self.done = true;
            return Progress::Done;
        }
        let underruns = self.producer.take_underruns();
        if underruns > 0 {
            log::warn!(
                "{} fell behind: {underruns} frames of silence",
                self.path.display()
            );
        }
        loop {
            if self.step() == Progress::Done {
                return Progress::Done;
            }
            if self.is_full() {
                return Progress::Running;
            }
        }
    }

    /// Whether this decoder has nothing more to do.
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// How many samples the ring holds when full: the bound on what this
    /// stream keeps resident.
    pub fn buffered_capacity(&self) -> usize {
        self.producer.capacity()
    }

    /// Whether decoded samples are waiting for room in the ring.
    fn is_full(&self) -> bool {
        self.pending_at < self.pending.len()
    }

    /// Pushes what is pending; if it all fit, decodes one more packet (or
    /// turns back at the loop) and pushes what it can of that.
    fn step(&mut self) -> Progress {
        if self.done {
            return Progress::Done;
        }
        self.push_pending();
        if self.is_full() {
            return Progress::Running;
        }
        if self.wrap_after_pending {
            self.wrap_after_pending = false;
            self.seek_to_loop();
        } else {
            self.decode_packet();
            self.push_pending();
        }
        self.progress()
    }

    fn push_pending(&mut self) {
        if self.is_full() {
            self.pending_at += self.producer.push(&self.pending[self.pending_at..]);
        }
    }

    fn progress(&self) -> Progress {
        if self.done {
            Progress::Done
        } else {
            Progress::Running
        }
    }

    /// Reads and decodes one packet into `pending`, or handles the end.
    fn decode_packet(&mut self) {
        self.pending.clear();
        self.pending_at = 0;

        let packet = match self.opened.reader.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => {
                self.end_of_file();
                return;
            }
            Err(SymphoniaError::ResetRequired) => {
                log::warn!(
                    "{} changes format part-way through; ending the stream there",
                    self.path.display()
                );
                self.end_of_file();
                return;
            }
            Err(e) => {
                log::warn!("{} stopped decoding: {e}", self.path.display());
                self.finish();
                return;
            }
        };
        if packet.track_id != self.opened.track_id {
            return;
        }

        // A block of its own: the decoded buffer borrows the decoder, and
        // finishing below needs the rest of `self`.
        let decoded = {
            let decoded = match self.opened.decoder.decode(&packet) {
                Ok(decoded) => Ok(decoded),
                // Recoverable, as in the whole-file decode: skip the packet.
                Err(SymphoniaError::IoError(_)) | Err(SymphoniaError::DecodeError(_)) => return,
                Err(e) => Err(e.to_string()),
            };
            match decoded {
                Err(e) => Err(e),
                Ok(decoded) if decoded.frames() == 0 => return,
                Ok(decoded) => {
                    let spec = (decoded.spec().rate(), decoded.spec().channels().count());
                    if spec == (self.rate, self.channels) {
                        decoded.copy_to_vec_interleaved(&mut self.decoded);
                        Ok(())
                    } else {
                        Err("it changes rate or channels part-way through".to_owned())
                    }
                }
            }
        };
        if let Err(e) = decoded {
            log::warn!("{} stopped decoding: {e}", self.path.display());
            self.finish();
            return;
        }

        let first = ts_to_frame(packet.pts.get(), self.opened.time_base, self.rate);
        let count = (self.decoded.len() / self.channels) as i64;
        let limit = match (self.looping, self.loop_end) {
            (true, Some(end)) => i64::try_from(end).unwrap_or(i64::MAX),
            _ => i64::MAX,
        };

        let from = (self.skip_until - first).clamp(0, count);
        let to = (limit.saturating_sub(first)).clamp(0, count);
        for frame in from..to {
            let base = frame as usize * self.channels;
            self.pending
                .extend_from_slice(&self.decoded[base..base + self.queued]);
        }
        if !self.pending.is_empty() {
            self.empty_laps = 0;
        }
        if first.saturating_add(count) >= limit {
            self.wrap_after_pending = true;
        }
    }

    /// The file ran out: back to the loop's start, or finished.
    fn end_of_file(&mut self) {
        if self.looping {
            self.seek_to_loop();
        } else {
            self.finish();
        }
    }

    /// Seeks back to just before the loop's start and discards up to it.
    ///
    /// A container that cannot seek is opened again from the beginning and
    /// decoded through to the loop — slower, and still exact.
    fn seek_to_loop(&mut self) {
        self.empty_laps = self.empty_laps.saturating_add(1);
        if self.empty_laps > 2 {
            log::warn!(
                "{} has nothing between its loop points; ending the stream",
                self.path.display()
            );
            self.finish();
            return;
        }
        let start = i64::try_from(self.loop_start).unwrap_or(i64::MAX);
        let before = self.loop_start.saturating_sub(PREROLL);
        let target = frame_to_ts(
            i64::try_from(before).unwrap_or(i64::MAX),
            self.opened.time_base,
            self.rate,
        );
        let seek = SeekTo::Timestamp {
            ts: Timestamp::new(target),
            track_id: self.opened.track_id,
        };
        let sought = self.opened.reader.seek(SeekMode::Accurate, seek).is_ok() || self.reopen();
        if !sought {
            self.finish();
            return;
        }
        self.opened.decoder.reset();
        self.skip_until = start;
    }

    /// Opens the source again from its first byte.
    fn reopen(&mut self) -> bool {
        match self
            .source
            .open()
            .and_then(|stream| open(stream, &self.path))
        {
            Ok(opened) => {
                self.opened = opened;
                true
            }
            Err(e) => {
                log::warn!("{e}; ending the stream");
                false
            }
        }
    }

    fn finish(&mut self) {
        self.done = true;
        self.producer.finish();
    }
}

impl std::fmt::Debug for StreamDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamDecoder")
            .field("path", &self.path)
            .field("looping", &self.looping)
            .field("done", &self.done)
            .finish()
    }
}
