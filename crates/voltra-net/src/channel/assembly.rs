//! One message being put back together from its fragments.

use crate::channel::FRAGMENT_SIZE;
use crate::wire::Malformed;

/// A fragment as it arrived, already parsed from its frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Piece<'a> {
    pub(crate) message: u16,
    pub(crate) index: u16,
    pub(crate) count: u16,
    pub(crate) bytes: &'a [u8],
}

impl Piece<'_> {
    /// Whether the piece is well-formed for a message of at most
    /// `max_fragments` fragments: in range, and every fragment but the last
    /// exactly [`FRAGMENT_SIZE`] long and the last of several not empty, so
    /// a message's length is fixed by its count and its last fragment — and
    /// its count by its length, as the sender counted it.
    pub(crate) fn check(&self, max_fragments: u16) -> Result<(), Malformed> {
        let fits = self.count >= 1
            && self.count <= max_fragments
            && self.index < self.count
            && self.bytes.len() <= FRAGMENT_SIZE
            && (self.index + 1 == self.count || self.bytes.len() == FRAGMENT_SIZE)
            && (self.count == 1 || self.index + 1 != self.count || !self.bytes.is_empty());
        if fits {
            Ok(())
        } else {
            Err(Malformed)
        }
    }
}

/// The fragments of one message so far.
///
/// Its buffer is sized from the first fragment's count — bounded by the
/// configuration's largest message, which [`Piece::check`] holds it to —
/// and the caller has already charged that size to the channel's budget.
#[derive(Debug)]
pub(crate) struct Assembly {
    count: u16,
    have: Vec<bool>,
    missing: u16,
    data: Vec<u8>,
    last_len: usize,
}

impl Assembly {
    pub(crate) fn new(count: u16) -> Self {
        Self {
            count,
            have: vec![false; usize::from(count)],
            missing: count,
            data: Vec::new(),
            last_len: 0,
        }
    }

    pub(crate) fn count(&self) -> u16 {
        self.count
    }

    /// Adds a checked piece of this message. A fragment it already has is
    /// ignored (a resend that crossed its ack); a count that disagrees with
    /// the first fragment's is a violation.
    pub(crate) fn add(&mut self, piece: &Piece<'_>) -> Result<(), Malformed> {
        if piece.count != self.count {
            return Err(Malformed);
        }
        let index = usize::from(piece.index);
        if self.have[index] {
            return Ok(());
        }
        if self.count == 1 {
            self.data = piece.bytes.to_vec();
        } else {
            if self.data.is_empty() {
                self.data = vec![0; usize::from(self.count) * FRAGMENT_SIZE];
            }
            let at = index * FRAGMENT_SIZE;
            self.data[at..at + piece.bytes.len()].copy_from_slice(piece.bytes);
            if piece.index + 1 == self.count {
                self.last_len = piece.bytes.len();
            }
        }
        self.have[index] = true;
        self.missing -= 1;
        Ok(())
    }

    pub(crate) fn is_whole(&self) -> bool {
        self.missing == 0
    }

    /// The message, once [`is_whole`](Self::is_whole).
    pub(crate) fn into_message(mut self) -> Vec<u8> {
        if self.count > 1 {
            self.data
                .truncate((usize::from(self.count) - 1) * FRAGMENT_SIZE + self.last_len);
        }
        self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(index: u16, count: u16, bytes: &[u8]) -> Piece<'_> {
        Piece {
            message: 0,
            index,
            count,
            bytes,
        }
    }

    #[test]
    fn fragments_in_any_order_make_the_message() {
        let full = [1u8; FRAGMENT_SIZE];
        let mut assembly = Assembly::new(3);
        for p in [piece(2, 3, b"end"), piece(0, 3, &full), piece(1, 3, &full)] {
            p.check(8).expect("well-formed");
            assembly.add(&p).expect("adds");
        }

        assert!(assembly.is_whole());
        let message = assembly.into_message();
        assert_eq!(message.len(), 2 * FRAGMENT_SIZE + 3);
        assert_eq!(&message[2 * FRAGMENT_SIZE..], b"end");
    }

    #[test]
    fn a_repeated_fragment_is_ignored() {
        let mut assembly = Assembly::new(2);
        let full = [0u8; FRAGMENT_SIZE];
        assembly.add(&piece(0, 2, &full)).expect("adds");
        assembly.add(&piece(0, 2, &full)).expect("ignored");
        assert!(!assembly.is_whole());
    }

    #[test]
    fn bad_pieces_are_refused() {
        let full = [0u8; FRAGMENT_SIZE];
        assert!(piece(0, 0, b"").check(8).is_err(), "no fragments");
        assert!(piece(3, 3, b"").check(8).is_err(), "index past count");
        assert!(piece(0, 9, &full).check(8).is_err(), "more than allowed");
        assert!(piece(0, 2, b"short").check(8).is_err(), "a short middle");
        assert!(
            piece(0, 1, &[0; FRAGMENT_SIZE + 1]).check(8).is_err(),
            "too long"
        );
        assert!(
            piece(0, 1, b"").check(8).is_ok(),
            "an empty message is fine"
        );
        assert!(piece(1, 2, b"").check(8).is_err(), "an empty last of two");

        let mut assembly = Assembly::new(2);
        assert!(assembly.add(&piece(1, 3, b"x")).is_err(), "count changed");
    }
}
