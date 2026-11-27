//! How often a server answers probes: per source and in all, so a flood
//! of probes — forged sources included — costs it a bounded trickle of
//! answers.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

/// The answer budget of a [`DiscoveryResponder`](super::DiscoveryResponder):
/// token buckets, one per source address and one for everyone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscoveryLimits {
    /// Answers a second to one source, once its burst is spent.
    pub per_source: f32,
    /// Answers one source gets at once.
    pub source_burst: f32,
    /// Answers a second in all.
    pub total: f32,
    /// Answers at once in all.
    pub total_burst: f32,
    /// Sources remembered at once. Past it, an idle one is forgotten to
    /// make room; with none idle, a new source is not answered.
    pub max_sources: usize,
}

impl Default for DiscoveryLimits {
    fn default() -> Self {
        Self {
            per_source: 2.0,
            source_burst: 4.0,
            total: 64.0,
            total_burst: 128.0,
            max_sources: 1024,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Bucket {
    tokens: f32,
    at: Duration,
}

impl Bucket {
    fn full(capacity: f32, now: Duration) -> Self {
        Self {
            tokens: capacity,
            at: now,
        }
    }

    fn refill(&mut self, rate: f32, capacity: f32, now: Duration) {
        let elapsed = now.saturating_sub(self.at).as_secs_f32();
        self.tokens = (self.tokens + elapsed * rate).min(capacity);
        self.at = self.at.max(now);
    }

    fn take(&mut self) -> bool {
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// The buckets.
#[derive(Debug)]
pub(super) struct Limiter {
    limits: DiscoveryLimits,
    sources: HashMap<IpAddr, Bucket>,
    total: Option<Bucket>,
}

impl Limiter {
    pub(super) fn new(limits: DiscoveryLimits) -> Self {
        let limits = DiscoveryLimits {
            per_source: limits.per_source.max(0.0),
            source_burst: limits.source_burst.max(1.0),
            total: limits.total.max(0.0),
            total_burst: limits.total_burst.max(1.0),
            max_sources: limits.max_sources.max(1),
        };
        Self {
            limits,
            sources: HashMap::new(),
            total: None,
        }
    }

    /// Whether `source` may be answered at `now`; spends a token of both
    /// buckets if so.
    pub(super) fn allow(&mut self, source: IpAddr, now: Duration) -> bool {
        let l = self.limits;
        let total = self
            .total
            .get_or_insert_with(|| Bucket::full(l.total_burst, now));
        total.refill(l.total, l.total_burst, now);
        if total.tokens < 1.0 {
            return false;
        }
        if !self.sources.contains_key(&source) && self.sources.len() >= l.max_sources {
            // Forget the sources whose buckets have refilled: they would
            // start full anyway.
            self.sources.retain(|_, bucket| {
                bucket.refill(l.per_source, l.source_burst, now);
                bucket.tokens < l.source_burst
            });
            if self.sources.len() >= l.max_sources {
                return false;
            }
        }
        let bucket = self
            .sources
            .entry(source)
            .or_insert_with(|| Bucket::full(l.source_burst, now));
        bucket.refill(l.per_source, l.source_burst, now);
        if !bucket.take() {
            return false;
        }
        // Checked above, so there is a token to take.
        total.take()
    }

    /// Sources remembered now.
    #[cfg(test)]
    pub(super) fn sources(&self) -> usize {
        self.sources.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(n: u32) -> IpAddr {
        IpAddr::from(n.to_be_bytes())
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn one_source_gets_its_burst_then_its_rate() {
        let mut limiter = Limiter::new(DiscoveryLimits::default());
        let answered = (0..100).filter(|_| limiter.allow(ip(1), ms(0))).count();
        assert_eq!(answered, 4, "the burst");

        // Half a second at two a second: one more.
        assert!(limiter.allow(ip(1), ms(500)));
        assert!(!limiter.allow(ip(1), ms(500)));
        // Another source is not held back by the first.
        assert!(limiter.allow(ip(2), ms(500)));
    }

    #[test]
    fn many_sources_share_the_total() {
        let mut limiter = Limiter::new(DiscoveryLimits {
            max_sources: 10_000,
            ..DiscoveryLimits::default()
        });
        let answered = (0..1000).filter(|&n| limiter.allow(ip(n), ms(0))).count();
        assert_eq!(answered, 128, "the total burst");
        let later = (1000..2000)
            .filter(|&n| limiter.allow(ip(n), ms(1000)))
            .count();
        assert_eq!(later, 64, "a second of the total rate");
    }

    #[test]
    fn the_table_of_sources_is_bounded_and_idle_ones_make_room() {
        let mut limiter = Limiter::new(DiscoveryLimits {
            max_sources: 8,
            total_burst: 1000.0,
            ..DiscoveryLimits::default()
        });
        for n in 0..8 {
            assert!(limiter.allow(ip(n), ms(0)));
        }
        assert!(!limiter.allow(ip(100), ms(0)), "no room, none idle");
        assert_eq!(limiter.sources(), 8);

        // Long after, every bucket has refilled: they make room.
        assert!(limiter.allow(ip(100), ms(10_000)));
        assert!(limiter.sources() <= 8);
    }
}
