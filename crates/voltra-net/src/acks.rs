//! Which packets arrived, both ways: what we tell the peer we received, and
//! what the peer's acks tell us of ours.
//!
//! Packet-level acks, as Gaffer On Games' "reliability over UDP" lays them
//! out and netcode, renet and Unity Transport all keep: each packet carries
//! the newest sequence received and a bit for each of the 32 before it, so
//! one surviving packet in 33 acknowledges everything. A message fragment
//! is acknowledged when a packet that carried it is.

use std::time::Duration;

use crate::packet::Acks;
use crate::sequence::newer;

/// How many sent packets are remembered for their acks. A packet older
/// than this is not acked any more — its fragments are simply resent.
const SENT_RING: usize = 1024;

/// A reliable fragment a packet carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Carried {
    pub(crate) channel: u8,
    pub(crate) message: u16,
    pub(crate) index: u16,
}

struct Sent {
    sequence: u16,
    at: Duration,
    acked: bool,
    carried: Vec<Carried>,
}

/// The packets we sent, by sequence, waiting to hear they arrived.
pub(crate) struct SentPackets {
    ring: Vec<Option<Sent>>,
}

impl SentPackets {
    pub(crate) fn new() -> Self {
        Self {
            ring: (0..SENT_RING).map(|_| None).collect(),
        }
    }

    pub(crate) fn record(&mut self, sequence: u16, at: Duration, carried: Vec<Carried>) {
        self.ring[usize::from(sequence) % SENT_RING] = Some(Sent {
            sequence,
            at,
            acked: false,
            carried,
        });
    }

    /// Applies the peer's `acks`: every packet they name that we still
    /// remember, once. `acked` is handed each fragment those packets
    /// carried. Answers when the newest one named was sent, if it was news —
    /// an RTT sample.
    pub(crate) fn apply(&mut self, acks: Acks, mut acked: impl FnMut(Carried)) -> Option<Duration> {
        let mut sample = None;
        for back in 0..=32u16 {
            if back > 0 && acks.bits & (1 << (back - 1)) == 0 {
                continue;
            }
            let sequence = acks.latest.wrapping_sub(back);
            let Some(sent) = self.ring[usize::from(sequence) % SENT_RING].as_mut() else {
                continue;
            };
            if sent.sequence != sequence || sent.acked {
                continue;
            }
            sent.acked = true;
            if back == 0 {
                sample = Some(sent.at);
            }
            for carried in sent.carried.drain(..) {
                acked(carried);
            }
        }
        sample
    }
}

/// The packets received from the peer, as the [`Acks`] we send back.
#[derive(Default)]
pub(crate) struct ReceivedPackets {
    acks: Option<Acks>,
}

impl ReceivedPackets {
    /// Notes `sequence` as received. Answers `false` for one already seen
    /// (a duplicate, whose frames need not be read again).
    pub(crate) fn note(&mut self, sequence: u16) -> bool {
        let Some(acks) = self.acks.as_mut() else {
            self.acks = Some(Acks {
                latest: sequence,
                bits: 0,
            });
            return true;
        };
        if newer(sequence, acks.latest) {
            let shift = u32::from(sequence.wrapping_sub(acks.latest));
            // Bit i stands for `latest - 1 - i`; the old latest becomes
            // bit `shift - 1`.
            let moved = if shift >= 33 {
                0
            } else {
                ((u64::from(acks.bits) << shift) | (1u64 << (shift - 1))) as u32
            };
            *acks = Acks {
                latest: sequence,
                bits: moved,
            };
            return true;
        }
        let back = acks.latest.wrapping_sub(sequence);
        if back == 0 {
            return false;
        }
        if back > 32 {
            // Too old to acknowledge; its fragments will come again. Read it
            // anyway: the channels drop what they have already.
            return true;
        }
        let bit = 1u32 << (back - 1);
        let fresh = acks.bits & bit == 0;
        acks.bits |= bit;
        fresh
    }

    /// What to tell the peer; `None` before anything arrived.
    pub(crate) fn acks(&self) -> Option<Acks> {
        self.acks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn received_packets_set_the_bits_behind_the_newest() {
        let mut received = ReceivedPackets::default();
        assert!(received.note(10));
        assert!(received.note(12));
        assert!(received.note(11));
        assert!(!received.note(11), "a duplicate");
        assert_eq!(
            received.acks(),
            Some(Acks {
                latest: 12,
                bits: 0b11
            })
        );
        assert!(received.note(50), "a jump past the bits clears them");
        assert_eq!(received.acks().map(|a| a.bits), Some(0));
    }

    #[test]
    fn the_bits_survive_a_wrap() {
        let mut received = ReceivedPackets::default();
        received.note(u16::MAX);
        received.note(0);
        assert_eq!(
            received.acks(),
            Some(Acks {
                latest: 0,
                bits: 0b1
            })
        );
    }

    #[test]
    fn an_ack_releases_what_its_packets_carried_once() {
        let mut sent = SentPackets::new();
        let piece = |message| Carried {
            channel: 0,
            message,
            index: 0,
        };
        sent.record(1, Duration::from_millis(10), vec![piece(1)]);
        sent.record(2, Duration::from_millis(20), vec![piece(2)]);
        sent.record(3, Duration::from_millis(30), vec![piece(3)]);

        let mut got = Vec::new();
        let sample = sent.apply(
            Acks {
                latest: 3,
                bits: 0b10,
            },
            |c| got.push(c.message),
        );
        assert_eq!(got, vec![3, 1], "2 was not named");
        assert_eq!(sample, Some(Duration::from_millis(30)));

        got.clear();
        sent.apply(
            Acks {
                latest: 3,
                bits: 0b11,
            },
            |c| got.push(c.message),
        );
        assert_eq!(got, vec![2], "only what was news");
    }
}
