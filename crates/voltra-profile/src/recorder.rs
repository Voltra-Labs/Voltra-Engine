//! One thread's frames: the spans and counters of the frame being recorded,
//! and the frames before it.

use std::collections::VecDeque;
use std::time::Duration;

use crate::clock::Instant;

/// How many closed frames a recorder keeps: ten seconds at 60 frames a
/// second, what a person scrubbing a graph or a tool asking for "the last
/// few seconds" wants, at a few kilobytes a frame.
pub const HISTORY: usize = 600;

/// A timed scope.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub name: &'static str,
    /// How many scopes were open around it: 0 at the top.
    pub depth: u16,
    /// When it opened, from the frame's start.
    pub start: Duration,
    pub duration: Duration,
}

/// One closed frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// Counts up for the life of the recorder.
    pub index: u64,
    /// From the previous frame's end to this one's.
    pub duration: Duration,
    /// In the order they closed: a child before its parent.
    pub spans: Vec<Span>,
    /// The last value each counter was given during the frame.
    pub counters: Vec<(&'static str, f64)>,
    /// GPU pass timings, in milliseconds, that came back during the frame —
    /// measured a frame or a few before, since the GPU runs behind.
    pub gpu: Vec<(&'static str, f64)>,
}

/// Records the frames of whoever owns it. [`crate::scope`] and the other
/// free functions use the calling thread's; a test makes its own.
#[derive(Debug)]
pub struct Recorder {
    enabled: bool,
    frame_start: Instant,
    depth: u16,
    spans: Vec<Span>,
    counters: Vec<(&'static str, f64)>,
    gpu: Vec<(&'static str, f64)>,
    history: VecDeque<Frame>,
    next_index: u64,
}

impl Default for Recorder {
    fn default() -> Self {
        Self {
            enabled: false,
            frame_start: Instant::now(),
            depth: 0,
            spans: Vec::new(),
            counters: Vec::new(),
            gpu: Vec::new(),
            history: VecDeque::new(),
            next_index: 0,
        }
    }
}

impl Recorder {
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Starts or stops recording. Starting begins a fresh frame now, so the
    /// first one recorded is not stretched back to when recording stopped.
    pub fn set_enabled(&mut self, enabled: bool, now: Instant) {
        if enabled && !self.enabled {
            self.frame_start = now;
            self.spans.clear();
            self.counters.clear();
            self.gpu.clear();
        }
        self.enabled = enabled;
    }

    /// Opens a scope: answers its depth, or `None` while disabled.
    pub fn open(&mut self) -> Option<u16> {
        if !self.enabled {
            return None;
        }
        let depth = self.depth;
        self.depth = self.depth.saturating_add(1);
        Some(depth)
    }

    /// Closes the scope [`open`](Self::open) answered `depth` for, opened at
    /// `start`.
    pub fn close(&mut self, name: &'static str, depth: u16, start: Instant, now: Instant) {
        self.depth = depth;
        if !self.enabled {
            return;
        }
        self.spans.push(Span {
            name,
            depth,
            // A scope opened before the frame did — one held across the
            // frame's end — counts from the frame's start.
            start: start.saturating_duration_since(self.frame_start),
            duration: now.saturating_duration_since(start),
        });
    }

    /// Sets `name` to `value` for this frame; a second call replaces it.
    pub fn count(&mut self, name: &'static str, value: f64) {
        if !self.enabled {
            return;
        }
        match self.counters.iter_mut().find(|(n, _)| *n == name) {
            Some(entry) => entry.1 = value,
            None => self.counters.push((name, value)),
        }
    }

    /// Records that the GPU took `ms` milliseconds over the pass `name`.
    pub fn gpu(&mut self, name: &'static str, ms: f64) {
        if self.enabled {
            self.gpu.push((name, ms));
        }
    }

    /// Closes the frame at `now` and starts the next.
    pub fn end_frame(&mut self, now: Instant) {
        if !self.enabled {
            return;
        }
        let frame = Frame {
            index: self.next_index,
            duration: now.saturating_duration_since(self.frame_start),
            spans: std::mem::take(&mut self.spans),
            counters: std::mem::take(&mut self.counters),
            gpu: std::mem::take(&mut self.gpu),
        };
        self.next_index += 1;
        self.history.push_back(frame);
        while self.history.len() > HISTORY {
            self.history.pop_front();
        }
        self.frame_start = now;
    }

    /// The closed frames, oldest first.
    pub fn frames(&self) -> &VecDeque<Frame> {
        &self.history
    }

    /// Forgets every frame, keeping whether it records.
    pub fn clear(&mut self) {
        self.history.clear();
        self.spans.clear();
        self.counters.clear();
        self.gpu.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    #[test]
    fn nested_scopes_close_child_first_with_their_depths_and_times() {
        let base = Instant::now();
        let mut recorder = Recorder::default();
        recorder.set_enabled(true, base);
        let outer = recorder.open().expect("enabled");
        let inner = recorder.open().expect("enabled");
        recorder.close("inner", inner, at(base, 2), at(base, 5));
        recorder.close("outer", outer, at(base, 1), at(base, 9));
        recorder.count("entities", 3.0);
        recorder.count("entities", 4.0);
        recorder.end_frame(at(base, 16));

        let frame = &recorder.frames()[0];
        assert_eq!(frame.duration, Duration::from_millis(16));
        assert_eq!(frame.spans[0].name, "inner");
        assert_eq!(frame.spans[0].depth, 1);
        assert_eq!(frame.spans[0].duration, Duration::from_millis(3));
        assert_eq!(frame.spans[1].depth, 0);
        assert_eq!(frame.spans[1].start, Duration::from_millis(1));
        assert_eq!(frame.counters, [("entities", 4.0)], "the last value");
    }

    #[test]
    fn a_disabled_recorder_records_nothing_and_history_is_capped() {
        let base = Instant::now();
        let mut recorder = Recorder::default();
        assert_eq!(recorder.open(), None);
        recorder.end_frame(base);
        assert!(recorder.frames().is_empty());

        recorder.set_enabled(true, base);
        for i in 0..(HISTORY as u64 + 5) {
            recorder.end_frame(at(base, i + 1));
        }
        assert_eq!(recorder.frames().len(), HISTORY);
        assert_eq!(recorder.frames()[0].index, 5, "the oldest went first");
    }
}
