//! A channel's receiving half: fragments in, whole messages out — once
//! each, and in order where the channel promises it.
//!
//! Everything here is fed from the wire. Ids are taken only inside the
//! window an honest sender can be in, every buffer is charged to the
//! channel's budget before it is allocated, and anything outside those
//! bounds is a [`Malformed`] the connection cuts the peer off for.

use crate::channel::assembly::{Assembly, Piece};
use crate::channel::{reserve, ChannelKind, RELIABLE_WINDOW};
use crate::sequence::newer;
use crate::wire::Malformed;

/// How many half-assembled messages a sequenced channel keeps; an older
/// one is dropped to make room — it was going to be stale anyway.
const MAX_SEQUENCED_PARTIALS: usize = 4;

enum Slot {
    Empty,
    Partial(Assembly),
    /// Whole, waiting for the messages before it (ordered channels), with
    /// the fragment count it was charged for.
    Whole(u16, Vec<u8>),
    /// Handed out ahead of the window's base (unordered channels).
    Delivered,
}

/// A reliable channel's window of message ids, from the oldest not yet
/// handed out.
struct Reliable {
    ordered: bool,
    base: u16,
    slots: Vec<Slot>,
}

struct Sequenced {
    /// The newest message handed out.
    last: Option<u16>,
    partials: Vec<(u16, Assembly)>,
}

enum Kind {
    Reliable(Reliable),
    Sequenced(Sequenced),
}

pub(crate) struct ReceiveChannel {
    kind: Kind,
    max_fragments: u16,
    budget: usize,
    buffered: usize,
}

impl ReceiveChannel {
    pub(crate) fn new(kind: ChannelKind, max_fragments: u16, budget: usize) -> Self {
        let kind = match kind {
            ChannelKind::ReliableOrdered | ChannelKind::ReliableUnordered => {
                Kind::Reliable(Reliable {
                    ordered: kind == ChannelKind::ReliableOrdered,
                    base: 0,
                    slots: (0..RELIABLE_WINDOW).map(|_| Slot::Empty).collect(),
                })
            }
            ChannelKind::UnreliableSequenced => Kind::Sequenced(Sequenced {
                last: None,
                partials: Vec::new(),
            }),
        };
        Self {
            kind,
            max_fragments,
            budget,
            buffered: 0,
        }
    }

    /// Takes one fragment; every message it completes (and, on an ordered
    /// channel, every one it unblocks) is handed to `deliver`, in order.
    pub(crate) fn receive(
        &mut self,
        piece: &Piece<'_>,
        deliver: &mut impl FnMut(Vec<u8>),
    ) -> Result<(), Malformed> {
        piece.check(self.max_fragments)?;
        match &mut self.kind {
            Kind::Reliable(window) => {
                window.receive(piece, &mut self.buffered, self.budget, deliver)
            }
            Kind::Sequenced(state) => {
                state.receive(piece, &mut self.buffered, self.budget, deliver);
                Ok(())
            }
        }
    }

    /// Bytes held for messages not yet handed out.
    #[cfg(test)]
    pub(crate) fn buffered(&self) -> usize {
        self.buffered
    }
}

impl Reliable {
    fn receive(
        &mut self,
        piece: &Piece<'_>,
        buffered: &mut usize,
        budget: usize,
        deliver: &mut impl FnMut(Vec<u8>),
    ) -> Result<(), Malformed> {
        let ahead = piece.message.wrapping_sub(self.base);
        if ahead >= RELIABLE_WINDOW {
            // Behind the base: handed out already, and this is a resend
            // whose ack was lost. An honest sender is never further *ahead*
            // than its window, so anything else is a violation.
            let behind = self.base.wrapping_sub(piece.message);
            return if behind <= RELIABLE_WINDOW {
                Ok(())
            } else {
                Err(Malformed)
            };
        }
        let at = usize::from(piece.message % RELIABLE_WINDOW);
        let slot = &mut self.slots[at];
        let mut assembly = match std::mem::replace(slot, Slot::Empty) {
            Slot::Empty => {
                let cost = reserve(piece.count);
                if *buffered + cost > budget {
                    return Err(Malformed);
                }
                *buffered += cost;
                Assembly::new(piece.count)
            }
            Slot::Partial(assembly) => assembly,
            done @ (Slot::Whole(..) | Slot::Delivered) => {
                *slot = done;
                return Ok(());
            }
        };
        let added = assembly.add(piece);
        if added.is_err() || !assembly.is_whole() {
            *slot = Slot::Partial(assembly);
            return added;
        }
        let count = assembly.count();
        let message = assembly.into_message();
        if self.ordered {
            *slot = Slot::Whole(count, message);
            loop {
                let at = usize::from(self.base % RELIABLE_WINDOW);
                if !matches!(self.slots[at], Slot::Whole(..)) {
                    break;
                }
                if let Slot::Whole(count, next) =
                    std::mem::replace(&mut self.slots[at], Slot::Empty)
                {
                    *buffered -= reserve(count);
                    deliver(next);
                }
                self.base = self.base.wrapping_add(1);
            }
        } else {
            *buffered -= reserve(count);
            deliver(message);
            *slot = Slot::Delivered;
            while let Slot::Delivered = self.slots[usize::from(self.base % RELIABLE_WINDOW)] {
                self.slots[usize::from(self.base % RELIABLE_WINDOW)] = Slot::Empty;
                self.base = self.base.wrapping_add(1);
            }
        }
        Ok(())
    }
}

impl Sequenced {
    fn receive(
        &mut self,
        piece: &Piece<'_>,
        buffered: &mut usize,
        budget: usize,
        deliver: &mut impl FnMut(Vec<u8>),
    ) {
        if self.last.is_some_and(|last| !newer(piece.message, last)) {
            return;
        }
        if piece.count == 1 {
            self.accept(piece.message, piece.bytes.to_vec(), buffered, deliver);
            return;
        }
        let place = match self
            .partials
            .iter()
            .position(|(id, _)| *id == piece.message)
        {
            Some(place) => place,
            None => {
                let cost = reserve(piece.count);
                // Make room by dropping the oldest partials: losing them is
                // what this channel allows.
                while !self.partials.is_empty()
                    && (self.partials.len() >= MAX_SEQUENCED_PARTIALS || *buffered + cost > budget)
                {
                    let (_, old) = self.partials.remove(0);
                    *buffered -= reserve(old.count());
                }
                if *buffered + cost > budget {
                    return;
                }
                *buffered += cost;
                self.partials
                    .push((piece.message, Assembly::new(piece.count)));
                self.partials.len() - 1
            }
        };
        let assembly = &mut self.partials[place].1;
        if assembly.add(piece).is_err() {
            // A count that changed mid-message: drop the message, as a
            // sequenced channel may.
            let (_, old) = self.partials.remove(place);
            *buffered -= reserve(old.count());
            return;
        }
        if assembly.is_whole() {
            let (id, whole) = self.partials.remove(place);
            *buffered -= reserve(whole.count());
            self.accept(id, whole.into_message(), buffered, deliver);
        }
    }

    fn accept(
        &mut self,
        id: u16,
        message: Vec<u8>,
        buffered: &mut usize,
        deliver: &mut impl FnMut(Vec<u8>),
    ) {
        self.last = Some(id);
        // Partials older than what was just handed out can only be stale.
        self.partials.retain(|(partial, assembly)| {
            let keep = newer(*partial, id);
            if !keep {
                *buffered -= reserve(assembly.count());
            }
            keep
        });
        deliver(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::FRAGMENT_SIZE;

    fn piece(message: u16, index: u16, count: u16, bytes: &[u8]) -> Piece<'_> {
        Piece {
            message,
            index,
            count,
            bytes,
        }
    }

    fn feed(channel: &mut ReceiveChannel, pieces: &[Piece<'_>]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for p in pieces {
            channel
                .receive(p, &mut |m| out.push(m))
                .expect("well-formed");
        }
        out
    }

    #[test]
    fn ordered_holds_back_until_the_gap_fills_then_hands_out_in_order() {
        let mut channel = ReceiveChannel::new(ChannelKind::ReliableOrdered, 8, 1 << 20);
        let out = feed(&mut channel, &[piece(1, 0, 1, b"b"), piece(2, 0, 1, b"c")]);
        assert!(out.is_empty());
        let out = feed(&mut channel, &[piece(0, 0, 1, b"a")]);
        assert_eq!(out, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
        assert_eq!(channel.buffered(), 0);
    }

    #[test]
    fn unordered_hands_out_at_once_and_each_only_once() {
        let mut channel = ReceiveChannel::new(ChannelKind::ReliableUnordered, 8, 1 << 20);
        let out = feed(
            &mut channel,
            &[
                piece(1, 0, 1, b"b"),
                piece(1, 0, 1, b"b"),
                piece(0, 0, 1, b"a"),
                piece(1, 0, 1, b"b"),
                piece(0, 0, 1, b"a"),
            ],
        );
        assert_eq!(out, vec![b"b".to_vec(), b"a".to_vec()]);
    }

    #[test]
    fn ids_wrap_round_without_losing_order() {
        let mut channel = ReceiveChannel::new(ChannelKind::ReliableOrdered, 8, 1 << 20);
        let mut out = Vec::new();
        for id in 0..70_000u32 {
            let id = id as u16;
            channel
                .receive(&piece(id, 0, 1, &id.to_le_bytes()), &mut |m| out.push(m))
                .expect("in the window");
        }
        assert_eq!(out.len(), 70_000);
        assert_eq!(out[65_536], 0u16.to_le_bytes().to_vec());
    }

    #[test]
    fn an_id_far_ahead_of_the_window_is_a_violation() {
        let mut channel = ReceiveChannel::new(ChannelKind::ReliableOrdered, 8, 1 << 20);
        let result = channel.receive(&piece(RELIABLE_WINDOW, 0, 1, b"x"), &mut |_| {});
        assert_eq!(result, Err(Malformed));
        let result = channel.receive(&piece(30_000, 0, 1, b"x"), &mut |_| {});
        assert_eq!(result, Err(Malformed));
    }

    #[test]
    fn holding_more_than_the_budget_is_a_violation() {
        let mut channel = ReceiveChannel::new(ChannelKind::ReliableOrdered, 8, 2 * FRAGMENT_SIZE);
        // Message 0 never comes, so 1 and 2 are held; a third is too many.
        feed(&mut channel, &[piece(1, 0, 1, b"b"), piece(2, 0, 1, b"c")]);
        let result = channel.receive(&piece(3, 0, 1, b"d"), &mut |_| {});
        assert_eq!(result, Err(Malformed));
    }

    #[test]
    fn sequenced_drops_what_is_older_than_the_newest_handed_out() {
        let mut channel = ReceiveChannel::new(ChannelKind::UnreliableSequenced, 8, 1 << 20);
        let out = feed(
            &mut channel,
            &[
                piece(5, 0, 1, b"5"),
                piece(3, 0, 1, b"3"),
                piece(6, 0, 1, b"6"),
                piece(6, 0, 1, b"6"),
            ],
        );
        assert_eq!(out, vec![b"5".to_vec(), b"6".to_vec()]);
    }

    #[test]
    fn a_sequenced_message_in_fragments_is_whole_or_nothing() {
        let full = [1u8; FRAGMENT_SIZE];
        let mut channel = ReceiveChannel::new(ChannelKind::UnreliableSequenced, 8, 1 << 20);
        // Message 0 loses its second fragment; message 1 arrives whole.
        let out = feed(
            &mut channel,
            &[
                piece(0, 0, 2, &full),
                piece(1, 1, 2, b"tail"),
                piece(1, 0, 2, &full),
            ],
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), FRAGMENT_SIZE + 4);
        assert_eq!(channel.buffered(), 0, "the stale partial was let go");
    }
}
