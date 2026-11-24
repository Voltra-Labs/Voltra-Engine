//! Reading and writing the bytes of a packet, every read bounds-checked.
//!
//! Hand-written rather than serde for the packet framing: every length on
//! the wire is checked against what is left before anything is sliced or
//! allocated, and a short packet is an `Err`, never an index panic.
//! Little-endian throughout.

/// A packet that does not parse: too short, a length past its end, a field
/// out of range. Never more specific than that — a peer that sent one is
/// either broken or hostile, and either way gets no help.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Malformed;

/// A cursor over received bytes.
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub(crate) fn bytes(&mut self, len: usize) -> Result<&'a [u8], Malformed> {
        if len > self.bytes.len() {
            return Err(Malformed);
        }
        let (taken, rest) = self.bytes.split_at(len);
        self.bytes = rest;
        Ok(taken)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Malformed> {
        let mut out = [0; N];
        out.copy_from_slice(self.bytes(N)?);
        Ok(out)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, Malformed> {
        Ok(self.array::<1>()?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, Malformed> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, Malformed> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, Malformed> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}

/// Appends fields to a packet being built.
pub(crate) struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.bytes.len()
    }

    pub(crate) fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(crate) fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    /// Zeroes up to `len` bytes in all.
    pub(crate) fn pad_to(&mut self, len: usize) {
        if self.bytes.len() < len {
            self.bytes.resize(len, 0);
        }
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_written_reads_back() {
        let mut w = Writer::with_capacity(16);
        w.u8(7);
        w.u16(0xBEEF);
        w.u32(0xDEAD_BEEF);
        w.u64(u64::MAX - 1);
        w.bytes(b"hi");
        let bytes = w.finish();

        let mut r = Reader::new(&bytes);
        assert_eq!(r.u8(), Ok(7));
        assert_eq!(r.u16(), Ok(0xBEEF));
        assert_eq!(r.u32(), Ok(0xDEAD_BEEF));
        assert_eq!(r.u64(), Ok(u64::MAX - 1));
        assert_eq!(r.bytes(2), Ok(&b"hi"[..]));
        assert!(r.is_empty());
    }

    #[test]
    fn a_read_past_the_end_is_malformed_not_a_panic() {
        let mut r = Reader::new(&[1, 2, 3]);
        assert_eq!(r.u32(), Err(Malformed));
        assert_eq!(r.bytes(usize::MAX), Err(Malformed));
        assert_eq!(r.remaining(), 3, "a failed read takes nothing");
    }
}
