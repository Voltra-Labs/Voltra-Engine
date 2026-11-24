//! A channel's sending half: messages split into fragments, kept until
//! acknowledged (reliable) or sent once (sequenced).

use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use crate::channel::{fragments_for, reserve, ChannelKind, FRAGMENT_SIZE, RELIABLE_WINDOW};

/// A message waiting to be sent or acknowledged.
struct Outgoing {
    id: u16,
    data: Rc<[u8]>,
    count: u16,
    /// Per fragment: when it was last sent, `None` before the first time.
    sent: Vec<Option<Duration>>,
    acked: Vec<bool>,
    unacked: u16,
}

/// A fragment the next packet may carry.
pub(crate) struct Due {
    pub(crate) message: u16,
    pub(crate) index: u16,
    pub(crate) count: u16,
    pub(crate) data: Rc<[u8]>,
}

impl Due {
    pub(crate) fn bytes(&self) -> &[u8] {
        let start = usize::from(self.index) * FRAGMENT_SIZE;
        let end = (start + FRAGMENT_SIZE).min(self.data.len());
        &self.data[start.min(end)..end]
    }
}

/// Why a message could not be queued on a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refused {
    /// Past the window or the byte budget: the peer has not caught up.
    Full,
}

pub(crate) struct SendChannel {
    kind: ChannelKind,
    next_id: u16,
    /// Reliable: every message from the oldest unacknowledged on, in id
    /// order, so a message's place is its id less the front's. Sequenced:
    /// what this flush sends, then forgotten.
    queue: VecDeque<Outgoing>,
    queued_bytes: usize,
    budget: usize,
}

impl SendChannel {
    pub(crate) fn new(kind: ChannelKind, budget: usize) -> Self {
        Self {
            kind,
            next_id: 0,
            queue: VecDeque::new(),
            queued_bytes: 0,
            budget,
        }
    }

    /// Queues `data`, already checked against the largest message.
    pub(crate) fn push(&mut self, data: &[u8]) -> Result<(), Refused> {
        let count = fragments_for(data.len()) as u16;
        let cost = reserve(count);
        let window = self.kind.is_reliable() && self.queue.len() >= usize::from(RELIABLE_WINDOW);
        if window || self.queued_bytes + cost > self.budget {
            return Err(Refused::Full);
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.queued_bytes += cost;
        self.queue.push_back(Outgoing {
            id,
            data: data.into(),
            count,
            sent: vec![None; usize::from(count)],
            acked: vec![false; usize::from(count)],
            unacked: count,
        });
        Ok(())
    }

    /// Fragments to send now: on a reliable channel, every one not yet
    /// acknowledged that was never sent or was sent `resend_after` ago; on a
    /// sequenced one, everything queued. `take` is called with each and
    /// answers whether it went into a packet; the first refusal ends the
    /// walk, so the channel's order is kept for the next flush. Answers how
    /// many fragments were sent again.
    pub(crate) fn drain_due(
        &mut self,
        now: Duration,
        resend_after: Duration,
        mut take: impl FnMut(&Due) -> bool,
    ) -> u64 {
        let mut resent = 0;
        let reliable = self.kind.is_reliable();
        'messages: for message in &mut self.queue {
            for index in 0..message.count {
                let i = usize::from(index);
                if message.acked[i] {
                    continue;
                }
                let due = match message.sent[i] {
                    None => true,
                    Some(at) => reliable && now.saturating_sub(at) >= resend_after,
                };
                if !due {
                    continue;
                }
                let piece = Due {
                    message: message.id,
                    index,
                    count: message.count,
                    data: message.data.clone(),
                };
                if !take(&piece) {
                    break 'messages;
                }
                if message.sent[i].is_some() {
                    resent += 1;
                }
                message.sent[i] = Some(now);
            }
        }
        if !reliable {
            // Sent once or not at all: a sequenced message that did not fit
            // this flush is stale by the next.
            let dropped = self.queue.iter().filter(|m| m.sent.contains(&None)).count();
            if dropped > 0 {
                log::debug!("{dropped} unreliable message(s) did not fit this flush");
            }
            self.queue.clear();
            self.queued_bytes = 0;
        }
        resent
    }

    /// The peer has fragment `index` of message `id`.
    pub(crate) fn ack(&mut self, id: u16, index: u16) {
        let Some(front) = self.queue.front() else {
            return;
        };
        let place = usize::from(id.wrapping_sub(front.id));
        let Some(message) = self.queue.get_mut(place) else {
            return;
        };
        let i = usize::from(index);
        if message.id != id || i >= message.acked.len() || message.acked[i] {
            return;
        }
        message.acked[i] = true;
        message.unacked -= 1;
        while self.queue.front().is_some_and(|m| m.unacked == 0) {
            if let Some(done) = self.queue.pop_front() {
                self.queued_bytes -= reserve(done.count);
            }
        }
    }

    /// Whether anything is waiting to be sent or acknowledged.
    #[cfg(test)]
    pub(crate) fn is_idle(&self) -> bool {
        self.queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    fn due(channel: &mut SendChannel, now: Duration) -> Vec<(u16, u16, usize)> {
        let mut out = Vec::new();
        channel.drain_due(now, 100 * MS, |d| {
            out.push((d.message, d.index, d.bytes().len()));
            true
        });
        out
    }

    #[test]
    fn a_long_message_goes_as_fragments_and_is_resent_until_acked() {
        let mut channel = SendChannel::new(ChannelKind::ReliableOrdered, 1 << 20);
        channel.push(&[0; FRAGMENT_SIZE * 2 + 5]).expect("queues");

        assert_eq!(
            due(&mut channel, Duration::ZERO),
            vec![(0, 0, FRAGMENT_SIZE), (0, 1, FRAGMENT_SIZE), (0, 2, 5)]
        );
        assert!(due(&mut channel, 50 * MS).is_empty(), "not due again yet");
        channel.ack(0, 1);
        assert_eq!(
            due(&mut channel, 100 * MS),
            vec![(0, 0, FRAGMENT_SIZE), (0, 2, 5)]
        );
        channel.ack(0, 0);
        channel.ack(0, 2);
        assert!(channel.is_idle());
    }

    #[test]
    fn a_reliable_channel_refuses_past_its_window() {
        let mut channel = SendChannel::new(ChannelKind::ReliableUnordered, 1 << 30);
        for _ in 0..RELIABLE_WINDOW {
            channel.push(b"x").expect("fits");
        }
        assert_eq!(channel.push(b"x"), Err(Refused::Full));
        channel.ack(0, 0);
        channel
            .push(b"x")
            .expect("room again once the oldest is acked");
    }

    #[test]
    fn a_reliable_channel_refuses_past_its_budget() {
        let mut channel = SendChannel::new(ChannelKind::ReliableOrdered, 2 * FRAGMENT_SIZE);
        channel.push(b"a").expect("fits");
        channel.push(b"b").expect("fits");
        assert_eq!(channel.push(b"c"), Err(Refused::Full));
    }

    #[test]
    fn a_sequenced_message_is_sent_once_and_forgotten() {
        let mut channel = SendChannel::new(ChannelKind::UnreliableSequenced, 4 * FRAGMENT_SIZE);
        channel.push(b"pos").expect("queues");
        channel.push(b"pos2").expect("queues");

        assert_eq!(due(&mut channel, Duration::ZERO).len(), 2);
        assert!(channel.is_idle());
        assert!(due(&mut channel, Duration::from_secs(1)).is_empty());
    }

    #[test]
    fn acks_out_of_range_are_ignored() {
        let mut channel = SendChannel::new(ChannelKind::ReliableOrdered, 1 << 20);
        channel.ack(5, 0);
        channel.push(b"x").expect("queues");
        channel.ack(0, 9);
        channel.ack(300, 0);
        assert!(!channel.is_idle());
    }
}
