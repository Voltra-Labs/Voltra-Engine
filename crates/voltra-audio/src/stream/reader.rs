//! The audio thread's end of a stream: frames out of the ring, one at a
//! time, interpolated the way a resident voice's are.

use super::ring::Consumer;

/// The reading end of one playing stream, handed to its voice.
#[derive(Debug)]
pub struct Feed {
    consumer: Consumer,
    /// Samples per frame in the ring: one or two.
    channels: u16,
}

impl Feed {
    pub(crate) fn new(consumer: Consumer, channels: u16) -> Self {
        Self {
            consumer,
            channels: channels.clamp(1, 2),
        }
    }

    /// How many samples are queued.
    pub fn queued(&self) -> usize {
        self.consumer.len()
    }

    /// One frame as left and right, or why there is none.
    fn pop(&mut self) -> Pop {
        let mut frame = [0.0; 2];
        let width = usize::from(self.channels);
        if self.consumer.pop_into(&mut frame[..width]) {
            if width == 1 {
                frame[1] = frame[0];
            }
            Pop::Frame(frame)
        } else if self.consumer.is_drained() {
            Pop::End
        } else {
            self.consumer.note_underrun();
            Pop::Late
        }
    }
}

enum Pop {
    Frame([f32; 2]),
    /// The decoder has not caught up: a frame of silence, and the stream
    /// carries on from where it was once it does.
    Late,
    /// The decoder finished and everything it wrote has been read.
    End,
}

/// A voice's cursor through a stream: the two frames it is between and how
/// far between them.
///
/// The resident voice reads any frame by index; a stream can only be read
/// forward, so this keeps the pair the linear interpolation needs and pulls
/// the next frame from the ring each time the cursor crosses one.
#[derive(Debug)]
pub(crate) struct StreamReader {
    feed: Feed,
    current: [f32; 2],
    next: [f32; 2],
    /// Whether `current` and `next` came from the file rather than from an
    /// underrun or the end.
    current_real: bool,
    next_real: bool,
    fraction: f64,
    ended: bool,
}

impl StreamReader {
    /// A reader at the start of `feed`, taking its first two frames now.
    pub(crate) fn new(feed: Feed) -> Self {
        let mut reader = Self {
            feed,
            current: [0.0; 2],
            next: [0.0; 2],
            current_real: false,
            next_real: false,
            fraction: 0.0,
            ended: false,
        };
        reader.shift();
        reader.shift();
        reader
    }

    /// Left and right at the cursor.
    pub(crate) fn read(&self) -> (f32, f32) {
        let t = self.fraction as f32;
        let lerp = |side: usize| self.current[side] + (self.next[side] - self.current[side]) * t;
        (lerp(0), lerp(1))
    }

    /// Moves the cursor `step` frames on.
    pub(crate) fn advance(&mut self, step: f64) {
        self.fraction += step;
        while self.fraction >= 1.0 && !self.is_finished() {
            self.fraction -= 1.0;
            self.shift();
        }
    }

    /// Whether the stream has ended and its last frame has been played.
    pub(crate) fn is_finished(&self) -> bool {
        self.ended && !self.current_real
    }

    /// Gives the feed back, for retiring off the audio thread.
    pub(crate) fn into_feed(self) -> Feed {
        self.feed
    }

    fn shift(&mut self) {
        self.current = self.next;
        self.current_real = self.next_real;
        match self.feed.pop() {
            Pop::Frame(frame) => {
                self.next = frame;
                self.next_real = true;
            }
            Pop::Late => {
                self.next = [0.0; 2];
                self.next_real = false;
            }
            Pop::End => {
                self.next = [0.0; 2];
                self.next_real = false;
                self.ended = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::ring::ring;

    fn feed(samples: &[f32], channels: u16, finish: bool) -> Feed {
        let (mut producer, consumer) = ring(64);
        producer.push(samples);
        if finish {
            producer.finish();
        } else {
            // Kept alive so the stream is late rather than ended.
            std::mem::forget(producer);
        }
        Feed::new(consumer, channels)
    }

    #[test]
    fn a_stream_read_at_its_own_rate_plays_its_frames() {
        let mut reader = StreamReader::new(feed(&[0.1, 0.2, 0.3], 1, true));
        let mut heard = Vec::new();
        while !reader.is_finished() {
            heard.push(reader.read().0);
            reader.advance(1.0);
        }
        assert_eq!(heard, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn half_steps_interpolate() {
        let mut reader = StreamReader::new(feed(&[0.0, 1.0], 1, true));
        reader.advance(0.5);
        assert!((reader.read().0 - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_stereo_stream_keeps_its_sides() {
        let reader = StreamReader::new(feed(&[1.0, -1.0], 2, true));
        assert_eq!(reader.read(), (1.0, -1.0));
    }

    #[test]
    fn a_late_stream_is_silent_and_not_finished() {
        let mut reader = StreamReader::new(feed(&[], 1, false));
        assert_eq!(reader.read(), (0.0, 0.0));
        reader.advance(4.0);
        assert!(!reader.is_finished(), "the decoder may still catch up");
    }
}
