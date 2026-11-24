//! A fixed-size queue of samples between one writer and one reader that
//! never locks and never allocates after construction.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;

/// The memory both ends share.
///
/// Samples are stored as `f32` bits in `AtomicU32`s rather than in an
/// `UnsafeCell<[f32]>`: a relaxed atomic load or store of a `u32` is a plain
/// move on every target this engine builds for, so the queue costs nothing
/// over the unsafe version and has no `unsafe` to audit. The ordering that
/// matters is on the two counters — the writer publishes samples with a
/// `Release` store of `written`, and the reader sees them after an `Acquire`
/// load of it; `read` the same way in the other direction for the space.
#[derive(Debug)]
struct Shared {
    slots: Box<[AtomicU32]>,
    /// Samples ever written. Wraps at `usize::MAX`, which is fine: only the
    /// difference to `read` is ever used, taken with wrapping arithmetic.
    written: AtomicUsize,
    /// Samples ever read.
    read: AtomicUsize,
    /// Set by the writer when nothing more will come: a stream that reached
    /// the end of a file it does not loop.
    finished: AtomicBool,
    /// Set by the reader when it is dropped, so the writer stops decoding for
    /// a voice that is gone.
    closed: AtomicBool,
    /// Times the reader found the queue empty before the writer finished:
    /// the decoder fell behind and a gap was heard. Counted here and reported
    /// from the writer's side, because the reader runs where logging would
    /// allocate.
    underruns: AtomicUsize,
}

/// The writing end, held by whatever decodes.
#[derive(Debug)]
pub struct Producer {
    shared: Arc<Shared>,
}

/// The reading end, held by the voice on the audio thread.
#[derive(Debug)]
pub struct Consumer {
    shared: Arc<Shared>,
}

/// A queue holding up to `capacity` samples, floored at one.
pub fn ring(capacity: usize) -> (Producer, Consumer) {
    let capacity = capacity.max(1);
    let slots = (0..capacity).map(|_| AtomicU32::new(0)).collect();
    let shared = Arc::new(Shared {
        slots,
        written: AtomicUsize::new(0),
        read: AtomicUsize::new(0),
        finished: AtomicBool::new(false),
        closed: AtomicBool::new(false),
        underruns: AtomicUsize::new(0),
    });
    (
        Producer {
            shared: Arc::clone(&shared),
        },
        Consumer { shared },
    )
}

impl Shared {
    fn capacity(&self) -> usize {
        self.slots.len()
    }
}

impl Producer {
    /// How many samples could be pushed right now.
    pub fn space(&self) -> usize {
        let written = self.shared.written.load(Ordering::Relaxed);
        let read = self.shared.read.load(Ordering::Acquire);
        self.shared.capacity() - written.wrapping_sub(read)
    }

    /// Pushes as much of `samples` as fits and answers how much that was.
    pub fn push(&mut self, samples: &[f32]) -> usize {
        let count = samples.len().min(self.space());
        let written = self.shared.written.load(Ordering::Relaxed);
        let capacity = self.shared.capacity();
        for (offset, sample) in samples[..count].iter().enumerate() {
            let slot = written.wrapping_add(offset) % capacity;
            self.shared.slots[slot].store(sample.to_bits(), Ordering::Relaxed);
        }
        self.shared
            .written
            .store(written.wrapping_add(count), Ordering::Release);
        count
    }

    /// Says nothing more will be pushed. What is queued still plays.
    pub fn finish(&mut self) {
        self.shared.finished.store(true, Ordering::Release);
    }

    /// Whether the reading end has been dropped.
    pub fn is_closed(&self) -> bool {
        self.shared.closed.load(Ordering::Acquire)
    }

    /// How many samples the queue holds when full.
    pub fn capacity(&self) -> usize {
        self.shared.capacity()
    }

    /// Takes the count of empty reads since the last call.
    pub fn take_underruns(&self) -> usize {
        self.shared.underruns.swap(0, Ordering::Relaxed)
    }
}

impl Drop for Producer {
    /// A writer that goes away has finished, whether or not it said so: a
    /// reader waiting on it would otherwise wait forever.
    fn drop(&mut self) {
        self.finish();
    }
}

impl Consumer {
    /// How many samples are queued.
    pub fn len(&self) -> usize {
        let written = self.shared.written.load(Ordering::Acquire);
        let read = self.shared.read.load(Ordering::Relaxed);
        written.wrapping_sub(read)
    }

    /// Whether nothing is queued.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Fills `out` from the queue if it holds that many samples, and answers
    /// whether it did. All or nothing, so a frame is never split across a
    /// gap.
    pub fn pop_into(&mut self, out: &mut [f32]) -> bool {
        if self.len() < out.len() {
            return false;
        }
        let read = self.shared.read.load(Ordering::Relaxed);
        let capacity = self.shared.capacity();
        for (offset, sample) in out.iter_mut().enumerate() {
            let slot = read.wrapping_add(offset) % capacity;
            *sample = f32::from_bits(self.shared.slots[slot].load(Ordering::Relaxed));
        }
        self.shared
            .read
            .store(read.wrapping_add(out.len()), Ordering::Release);
        true
    }

    /// Whether the writer is done and everything it wrote has been read.
    pub fn is_drained(&self) -> bool {
        // `finished` first: a writer that pushes then finishes has published
        // its last samples before the flag, so seeing the flag and then an
        // empty queue means empty for good.
        self.shared.finished.load(Ordering::Acquire) && self.is_empty()
    }

    /// Records one empty read, for the writer's side to report.
    pub fn note_underrun(&self) {
        self.shared.underruns.fetch_add(1, Ordering::Relaxed);
    }
}

impl Drop for Consumer {
    fn drop(&mut self) {
        self.shared.closed.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_pushed_is_popped_in_order() {
        let (mut producer, mut consumer) = ring(4);
        assert_eq!(producer.push(&[1.0, 2.0, 3.0]), 3);
        let mut out = [0.0; 2];
        assert!(consumer.pop_into(&mut out));
        assert_eq!(out, [1.0, 2.0]);
        assert_eq!(consumer.len(), 1);
    }

    #[test]
    fn a_full_queue_takes_only_what_fits() {
        let (mut producer, _consumer) = ring(3);
        assert_eq!(producer.push(&[1.0; 5]), 3);
        assert_eq!(producer.space(), 0);
        assert_eq!(producer.push(&[1.0]), 0);
    }

    #[test]
    fn the_queue_wraps_around_its_end() {
        let (mut producer, mut consumer) = ring(3);
        let mut out = [0.0; 2];
        for round in 0..10 {
            let a = round as f32;
            assert_eq!(producer.push(&[a, a + 0.5]), 2);
            assert!(consumer.pop_into(&mut out));
            assert_eq!(out, [a, a + 0.5]);
        }
    }

    #[test]
    fn a_short_queue_pops_nothing_rather_than_half_a_frame() {
        let (mut producer, mut consumer) = ring(4);
        producer.push(&[1.0]);
        let mut out = [9.0; 2];
        assert!(!consumer.pop_into(&mut out));
        assert_eq!(out, [9.0, 9.0], "untouched");
        assert_eq!(consumer.len(), 1);
    }

    #[test]
    fn a_dropped_writer_drains_then_ends() {
        let (mut producer, mut consumer) = ring(4);
        producer.push(&[1.0]);
        drop(producer);
        assert!(!consumer.is_drained(), "one sample is still queued");
        let mut out = [0.0];
        assert!(consumer.pop_into(&mut out));
        assert!(consumer.is_drained());
    }

    #[test]
    fn a_dropped_reader_tells_the_writer_to_stop() {
        let (producer, consumer) = ring(4);
        assert!(!producer.is_closed());
        drop(consumer);
        assert!(producer.is_closed());
    }

    #[test]
    fn underruns_are_counted_and_taken() {
        let (producer, consumer) = ring(4);
        consumer.note_underrun();
        consumer.note_underrun();
        assert_eq!(producer.take_underruns(), 2);
        assert_eq!(producer.take_underruns(), 0);
    }

    #[test]
    fn the_two_ends_work_from_two_threads() {
        let (mut producer, mut consumer) = ring(64);
        let writer = std::thread::spawn(move || {
            let mut next = 0u32;
            while next < 10_000 {
                if producer.push(&[next as f32]) == 1 {
                    next += 1;
                } else {
                    std::thread::yield_now();
                }
            }
        });
        let mut expected = 0u32;
        let mut out = [0.0];
        while expected < 10_000 {
            if consumer.pop_into(&mut out) {
                assert_eq!(out[0], expected as f32);
                expected += 1;
            } else {
                std::thread::yield_now();
            }
        }
        writer.join().expect("the writer finishes");
    }
}
