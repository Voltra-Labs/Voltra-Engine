//! One interpolated component's recent values on a remote entity, by the
//! server tick each was true at.

use std::collections::VecDeque;
use std::sync::Arc;

use crate::wire::ServerTick;

/// The most samples kept: a second at 60 snapshots a second, far more than
/// any buffer draws behind.
const MAX_SAMPLES: usize = 64;

/// What to draw at a moment.
pub(crate) enum Pick<'a> {
    /// One value: the moment is past the newest, or before the oldest.
    Hold(&'a Arc<[u8]>),
    /// `t` of the way between two.
    Between(&'a Arc<[u8]>, &'a Arc<[u8]>, f32),
}

/// The samples, oldest first, ticks strictly increasing.
#[derive(Debug, Default, Clone)]
pub(crate) struct Samples(VecDeque<(ServerTick, Arc<[u8]>)>);

impl Samples {
    /// Adds the value true at `tick`. An older or equal tick is a late
    /// copy and changes nothing.
    pub(crate) fn push(&mut self, tick: ServerTick, value: Arc<[u8]>) {
        if self.0.back().is_some_and(|(newest, _)| *newest >= tick) {
            return;
        }
        self.0.push_back((tick, value));
        while self.0.len() > MAX_SAMPLES {
            self.0.pop_front();
        }
    }

    /// The newest value is still true at `tick`: a complete snapshot left
    /// the entity out because nothing about it changed.
    pub(crate) fn hold(&mut self, tick: ServerTick) {
        if let Some((_, newest)) = self.0.back() {
            let newest = newest.clone();
            self.push(tick, newest);
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }

    /// What to draw at `tick` (fractional), dropping the samples no later
    /// moment needs.
    pub(crate) fn at(&mut self, tick: f64) -> Option<Pick<'_>> {
        while self.0.len() >= 2 && f64::from(self.0[1].0) <= tick {
            self.0.pop_front();
        }
        let first = self.0.front()?;
        if tick <= f64::from(first.0) {
            return Some(Pick::Hold(&first.1));
        }
        match self.0.get(1) {
            Some(next) => {
                let span = f64::from(next.0 - first.0);
                let t = ((tick - f64::from(first.0)) / span).clamp(0.0, 1.0) as f32;
                Some(Pick::Between(&first.1, &next.1, t))
            }
            None => Some(Pick::Hold(&first.1)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(byte: u8) -> Arc<[u8]> {
        Arc::from(vec![byte])
    }

    #[test]
    fn a_moment_between_two_samples_is_between_them() {
        let mut samples = Samples::default();
        samples.push(10, value(0));
        samples.push(12, value(2));
        samples.push(11, value(9)); // late: ignored
        match samples.at(10.5) {
            Some(Pick::Between(a, b, t)) => {
                assert_eq!((a[0], b[0]), (0, 2));
                assert!((t - 0.25).abs() < 1e-6);
            }
            _ => panic!("between"),
        }
        assert!(matches!(samples.at(13.0), Some(Pick::Hold(v)) if v[0] == 2));
        assert!(matches!(samples.at(5.0), Some(Pick::Hold(v)) if v[0] == 2));
    }

    #[test]
    fn a_held_value_is_a_sample() {
        let mut samples = Samples::default();
        samples.push(1, value(7));
        samples.hold(5);
        samples.push(6, value(8));
        // Still at 7 until tick 5, then on to 8 — not smeared from tick 1.
        assert!(matches!(samples.at(4.0), Some(Pick::Between(a, b, _)) if a[0] == 7 && b[0] == 7));
        assert!(matches!(samples.at(5.5), Some(Pick::Between(a, b, _)) if a[0] == 7 && b[0] == 8));
    }
}
