//! A bad network on purpose: loss, latency, jitter and duplication applied
//! to any transport's sends.
//!
//! The wrapper shape is Unity Transport's `SimulatorPipelineStage` and
//! renet's conditioner: a stage between the connection and the socket, so
//! the same code that runs a test over [`MemoryNetwork`](crate::MemoryNetwork)
//! can, in the editor, slow down a real UDP socket. Applied on the way
//! *out*; wrap both ends for a link that is bad in both directions.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::Duration;

use crate::rng::Rng;
use crate::transport::{check_size, Datagram, Transport, TransportError};
use crate::Address;

/// The most datagrams held back for later; past it a send is dropped.
const MAX_DELAYED: usize = 65_536;

/// How a link misbehaves. The default is a perfect one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkConditions {
    /// One-way delay every datagram waits.
    pub latency: Duration,
    /// Up to this much more, uniformly at random per datagram — which is
    /// also what reorders them: one that waits less overtakes one sent
    /// before it.
    pub jitter: Duration,
    /// The chance, `0.0..=1.0`, that a datagram is lost.
    pub loss: f32,
    /// The chance that a datagram is delivered twice, each copy delayed on
    /// its own.
    pub duplicate: f32,
}

impl Default for LinkConditions {
    fn default() -> Self {
        Self {
            latency: Duration::ZERO,
            jitter: Duration::ZERO,
            loss: 0.0,
            duplicate: 0.0,
        }
    }
}

/// What a [`Conditioned`] transport did to the traffic through it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConditionStats {
    /// Datagrams handed to the inner transport.
    pub delivered: u64,
    /// Datagrams lost on purpose.
    pub lost: u64,
    /// Extra copies made.
    pub duplicated: u64,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Delayed {
    at: Duration,
    /// Breaks ties in send order, so equal delays keep it.
    order: u64,
    to: Address,
    payload: Vec<u8>,
}

/// `T` with [`LinkConditions`] applied to what it sends.
pub struct Conditioned<T> {
    inner: T,
    conditions: LinkConditions,
    rng: Rng,
    now: Duration,
    order: u64,
    delayed: BinaryHeap<Reverse<Delayed>>,
    stats: ConditionStats,
}

impl<T: Transport> Conditioned<T> {
    /// Wraps `inner`, misbehaving as `conditions` say, rolling dice from
    /// `seed` so a run replays exactly.
    pub fn new(inner: T, conditions: LinkConditions, seed: u64) -> Self {
        Self {
            inner,
            conditions,
            rng: Rng::new(seed),
            now: Duration::ZERO,
            order: 0,
            delayed: BinaryHeap::new(),
            stats: ConditionStats::default(),
        }
    }

    /// How the link misbehaves now.
    pub fn conditions(&self) -> LinkConditions {
        self.conditions
    }

    /// Changes how the link misbehaves, from the next send on; what is
    /// already delayed keeps its time.
    pub fn set_conditions(&mut self, conditions: LinkConditions) {
        self.conditions = conditions;
    }

    /// What the conditions did so far.
    pub fn stats(&self) -> ConditionStats {
        self.stats
    }

    /// The transport underneath.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// The transport underneath, mutably.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    fn delay(&mut self) -> Duration {
        let jitter = self.conditions.jitter.as_secs_f64() * self.rng.unit();
        self.conditions.latency + Duration::from_secs_f64(jitter)
    }

    fn deliver(&mut self, to: Address, payload: &[u8]) -> Result<(), TransportError> {
        self.stats.delivered += 1;
        self.inner.send(to, payload)
    }

    fn release_due(&mut self) -> Result<(), TransportError> {
        while self
            .delayed
            .peek()
            .is_some_and(|Reverse(next)| next.at <= self.now)
        {
            let Some(Reverse(due)) = self.delayed.pop() else {
                break;
            };
            self.deliver(due.to, &due.payload)?;
        }
        Ok(())
    }
}

impl<T: Transport> Transport for Conditioned<T> {
    fn update(&mut self, now: Duration) -> Result<(), TransportError> {
        self.now = self.now.max(now);
        self.inner.update(now)?;
        self.release_due()
    }

    fn send(&mut self, to: Address, payload: &[u8]) -> Result<(), TransportError> {
        check_size(payload)?;
        if !self.inner.reaches(to) {
            return Err(TransportError::Unreachable(to));
        }
        if self.rng.chance(self.conditions.loss) {
            self.stats.lost += 1;
            return Ok(());
        }
        let copies = if self.rng.chance(self.conditions.duplicate) {
            self.stats.duplicated += 1;
            2
        } else {
            1
        };
        for _ in 0..copies {
            let delay = self.delay();
            if delay.is_zero() {
                self.deliver(to, payload)?;
            } else if self.delayed.len() < MAX_DELAYED {
                self.order += 1;
                self.delayed.push(Reverse(Delayed {
                    at: self.now + delay,
                    order: self.order,
                    to,
                    payload: payload.to_vec(),
                }));
            } else {
                self.stats.lost += 1;
            }
        }
        Ok(())
    }

    fn recv(&mut self) -> Result<Option<Datagram>, TransportError> {
        self.inner.recv()
    }

    fn reaches(&self, to: Address) -> bool {
        self.inner.reaches(to)
    }

    fn local_addr(&self) -> Option<Address> {
        self.inner.local_addr()
    }

    fn take_closed(&mut self) -> Option<Address> {
        self.inner.take_closed()
    }

    fn close(&mut self, peer: Address) {
        self.inner.close(peer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryNetwork;

    const MS: Duration = Duration::from_millis(1);

    fn drain(transport: &mut impl Transport) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while let Some(datagram) = transport.recv().expect("reads") {
            out.push(datagram.payload);
        }
        out
    }

    #[test]
    fn latency_holds_a_datagram_until_its_time() {
        let network = MemoryNetwork::new();
        let conditions = LinkConditions {
            latency: 100 * MS,
            ..Default::default()
        };
        let mut a = Conditioned::new(network.endpoint(), conditions, 1);
        let mut b = network.endpoint();

        a.send(b.address(), b"late").expect("sends");
        a.update(99 * MS).expect("updates");
        assert!(drain(&mut b).is_empty(), "not yet");
        a.update(100 * MS).expect("updates");
        assert_eq!(drain(&mut b), vec![b"late".to_vec()]);
    }

    #[test]
    fn loss_drops_about_its_share_and_counts_them() {
        let network = MemoryNetwork::new();
        let conditions = LinkConditions {
            loss: 0.3,
            ..Default::default()
        };
        let mut a = Conditioned::new(network.endpoint(), conditions, 2);
        let mut b = network.endpoint();

        for i in 0..1000u16 {
            a.send(b.address(), &i.to_le_bytes()).expect("sends");
        }

        let arrived = drain(&mut b).len();
        assert!((600..800).contains(&arrived), "{arrived}");
        assert_eq!(a.stats().lost as usize, 1000 - arrived);
    }

    #[test]
    fn jitter_reorders_and_duplication_repeats() {
        let network = MemoryNetwork::new();
        let conditions = LinkConditions {
            latency: 10 * MS,
            jitter: 50 * MS,
            duplicate: 0.2,
            ..Default::default()
        };
        let mut a = Conditioned::new(network.endpoint(), conditions, 3);
        let mut b = network.endpoint();

        for i in 0..200u8 {
            a.send(b.address(), &[i]).expect("sends");
        }
        a.update(100 * MS).expect("updates");

        let arrived: Vec<u8> = drain(&mut b).into_iter().map(|p| p[0]).collect();
        assert!(arrived.len() > 200, "some arrived twice");
        assert!(
            arrived.windows(2).any(|w| w[0] > w[1]),
            "some overtook others"
        );
        let mut unique = arrived.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 200, "and none was lost");
    }

    #[test]
    fn the_same_seed_misbehaves_the_same_way() {
        let run = |seed| {
            let network = MemoryNetwork::new();
            let conditions = LinkConditions {
                loss: 0.5,
                ..Default::default()
            };
            let mut a = Conditioned::new(network.endpoint(), conditions, seed);
            let mut b = network.endpoint();
            for i in 0..64u8 {
                a.send(b.address(), &[i]).expect("sends");
            }
            drain(&mut b)
        };

        assert_eq!(run(9), run(9));
        assert_ne!(run(9), run(10));
    }
}
