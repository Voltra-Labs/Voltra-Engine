//! Sixteen-bit sequence numbers that wrap: packet numbers and message ids.
//!
//! Sixty-five thousand numbers wrap in under twenty minutes at sixty
//! packets a second, so "newer" is the shorter way round the circle —
//! Gaffer On Games' `sequence_greater_than`, which every netcode since uses.

/// Whether `a` comes after `b`, the shorter way round.
pub(crate) fn newer(a: u16, b: u16) -> bool {
    a != b && a.wrapping_sub(b) < 0x8000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_wraps_the_short_way_round() {
        assert!(newer(1, 0));
        assert!(!newer(0, 1));
        assert!(newer(0, u16::MAX), "0 follows 65535");
        assert!(!newer(u16::MAX, 0));
        assert!(!newer(5, 5));
        assert!(newer(0x7FFF, 0));
        assert!(!newer(0x8000, 0), "half way round is not newer");
    }
}
