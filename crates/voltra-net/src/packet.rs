//! The packets on the wire, and how each is read and written.
//!
//! ```text
//! request    1 | version u16 | fingerprint u64 | nonce u64 | zeros to 64 bytes
//! challenge  2 | nonce u64 | cookie u64
//! response   3 | nonce u64 | cookie u64 | zeros to 64 bytes
//! accept     4 | nonce u64 | session u64 | peer u32
//! deny       5 | nonce u64 | reason u8
//! payload    6 | session u64 | sequence u16 | ack u16 | ack bits u32 | frame*
//! disconnect 7 | session u64 | reason u8
//!
//! frame      channel u8 | message u16 | fragment u16 | fragments u16 | len u16 | bytes
//! ```
//!
//! The handshake is netcode's shape without its encryption: a request, a
//! challenge carrying a cookie the server can check without having kept
//! anything, the cookie echoed back. The two packets a stranger can make
//! the server answer are padded to at least the size of the answer, so
//! the server is no amplifier for a spoofed source address.

use crate::channel::assembly::Piece;
use crate::wire::{Malformed, Reader, Writer};
use crate::MAX_DATAGRAM;

/// The size a request and a response are padded to, and the least a server
/// accepts: larger than any answer to them.
pub(crate) const PADDED: usize = 64;

/// A frame's header, before its bytes.
pub(crate) const FRAME_HEADER: usize = 1 + 2 + 2 + 2 + 2;

const REQUEST: u8 = 1;
const CHALLENGE: u8 = 2;
const RESPONSE: u8 = 3;
const ACCEPT: u8 = 4;
const DENY: u8 = 5;
const PAYLOAD: u8 = 6;
const DISCONNECT: u8 = 7;

/// The acknowledgements a payload packet carries: the newest packet
/// received, and a bit for each of the 32 before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Acks {
    pub(crate) latest: u16,
    pub(crate) bits: u32,
}

/// A packet as read: handshake fields, or a payload whose frames are read
/// on demand by [`frames`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Packet<'a> {
    Request {
        version: u16,
        fingerprint: u64,
        nonce: u64,
    },
    Challenge {
        nonce: u64,
        cookie: u64,
    },
    Response {
        nonce: u64,
        cookie: u64,
    },
    Accept {
        nonce: u64,
        session: u64,
        peer: u32,
    },
    Deny {
        nonce: u64,
        reason: u8,
    },
    Payload {
        session: u64,
        sequence: u16,
        acks: Acks,
        frames: &'a [u8],
    },
    Disconnect {
        session: u64,
        reason: u8,
    },
}

impl<'a> Packet<'a> {
    /// Reads one datagram. Anything short, padded short, or of an unknown
    /// kind is [`Malformed`].
    pub(crate) fn read(bytes: &'a [u8]) -> Result<Self, Malformed> {
        let mut r = Reader::new(bytes);
        let packet = match r.u8()? {
            REQUEST if bytes.len() >= PADDED => Self::Request {
                version: r.u16()?,
                fingerprint: r.u64()?,
                nonce: r.u64()?,
            },
            CHALLENGE => Self::Challenge {
                nonce: r.u64()?,
                cookie: r.u64()?,
            },
            RESPONSE if bytes.len() >= PADDED => Self::Response {
                nonce: r.u64()?,
                cookie: r.u64()?,
            },
            ACCEPT => Self::Accept {
                nonce: r.u64()?,
                session: r.u64()?,
                peer: r.u32()?,
            },
            DENY => Self::Deny {
                nonce: r.u64()?,
                reason: r.u8()?,
            },
            PAYLOAD => {
                let session = r.u64()?;
                let sequence = r.u16()?;
                let acks = Acks {
                    latest: r.u16()?,
                    bits: r.u32()?,
                };
                let frames = r.bytes(r.remaining())?;
                return Ok(Self::Payload {
                    session,
                    sequence,
                    acks,
                    frames,
                });
            }
            DISCONNECT => Self::Disconnect {
                session: r.u64()?,
                reason: r.u8()?,
            },
            _ => return Err(Malformed),
        };
        Ok(packet)
    }

    /// The bytes of a handshake or disconnect packet. A payload is built
    /// with [`PayloadWriter`] instead.
    pub(crate) fn write(&self) -> Vec<u8> {
        let mut w = Writer::with_capacity(PADDED);
        match *self {
            Self::Request {
                version,
                fingerprint,
                nonce,
            } => {
                w.u8(REQUEST);
                w.u16(version);
                w.u64(fingerprint);
                w.u64(nonce);
                w.pad_to(PADDED);
            }
            Self::Challenge { nonce, cookie } => {
                w.u8(CHALLENGE);
                w.u64(nonce);
                w.u64(cookie);
            }
            Self::Response { nonce, cookie } => {
                w.u8(RESPONSE);
                w.u64(nonce);
                w.u64(cookie);
                w.pad_to(PADDED);
            }
            Self::Accept {
                nonce,
                session,
                peer,
            } => {
                w.u8(ACCEPT);
                w.u64(nonce);
                w.u64(session);
                w.u32(peer);
            }
            Self::Deny { nonce, reason } => {
                w.u8(DENY);
                w.u64(nonce);
                w.u8(reason);
            }
            Self::Payload {
                session,
                sequence,
                acks,
                frames,
            } => {
                let mut p = PayloadWriter::new(session, sequence, acks);
                p.w.bytes(frames);
                return p.finish();
            }
            Self::Disconnect { session, reason } => {
                w.u8(DISCONNECT);
                w.u64(session);
                w.u8(reason);
            }
        }
        w.finish()
    }
}

/// A frame as read from a payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Frame<'a> {
    pub(crate) channel: u8,
    pub(crate) piece: Piece<'a>,
}

/// The frames of a payload, in order; the first that does not parse ends
/// the walk with [`Malformed`].
pub(crate) fn frames(bytes: &[u8]) -> impl Iterator<Item = Result<Frame<'_>, Malformed>> {
    let mut r = Reader::new(bytes);
    let mut failed = false;
    std::iter::from_fn(move || {
        if failed || r.is_empty() {
            return None;
        }
        let frame = (|| {
            let channel = r.u8()?;
            let message = r.u16()?;
            let index = r.u16()?;
            let count = r.u16()?;
            let len = usize::from(r.u16()?);
            let bytes = r.bytes(len)?;
            Ok(Frame {
                channel,
                piece: Piece {
                    message,
                    index,
                    count,
                    bytes,
                },
            })
        })();
        failed = frame.is_err();
        Some(frame)
    })
}

/// A payload packet being filled with frames, up to [`MAX_DATAGRAM`].
pub(crate) struct PayloadWriter {
    w: Writer,
}

impl PayloadWriter {
    pub(crate) fn new(session: u64, sequence: u16, acks: Acks) -> Self {
        let mut w = Writer::with_capacity(MAX_DATAGRAM);
        w.u8(PAYLOAD);
        w.u64(session);
        w.u16(sequence);
        w.u16(acks.latest);
        w.u32(acks.bits);
        Self { w }
    }

    /// Whether a frame of `len` bytes still fits.
    pub(crate) fn fits(&self, len: usize) -> bool {
        self.w.len() + FRAME_HEADER + len <= MAX_DATAGRAM
    }

    /// Appends a frame; the caller asked [`fits`](Self::fits) first.
    pub(crate) fn frame(&mut self, channel: u8, piece: &Piece<'_>) {
        debug_assert!(self.fits(piece.bytes.len()));
        self.w.u8(channel);
        self.w.u16(piece.message);
        self.w.u16(piece.index);
        self.w.u16(piece.count);
        // A fragment is at most FRAGMENT_SIZE, well inside a u16.
        self.w.u16(piece.bytes.len() as u16);
        self.w.bytes(piece.bytes);
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        self.w.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::FRAGMENT_SIZE;

    #[test]
    fn every_handshake_packet_reads_back_as_written() {
        let packets = [
            Packet::Request {
                version: 1,
                fingerprint: 2,
                nonce: 3,
            },
            Packet::Challenge {
                nonce: 4,
                cookie: 5,
            },
            Packet::Response {
                nonce: 6,
                cookie: 7,
            },
            Packet::Accept {
                nonce: 8,
                session: 9,
                peer: 10,
            },
            Packet::Deny {
                nonce: 11,
                reason: 3,
            },
            Packet::Disconnect {
                session: 12,
                reason: 2,
            },
        ];
        for packet in packets {
            let bytes = packet.write();
            assert_eq!(Packet::read(&bytes), Ok(packet));
        }
    }

    #[test]
    fn a_stranger_cannot_get_more_back_than_it_sent() {
        let request = Packet::Request {
            version: 1,
            fingerprint: 0,
            nonce: 0,
        }
        .write();
        let challenge = Packet::Challenge {
            nonce: 0,
            cookie: 0,
        }
        .write();
        let deny = Packet::Deny {
            nonce: 0,
            reason: 0,
        }
        .write();
        assert!(request.len() >= challenge.len() && request.len() >= deny.len());
        assert!(
            Packet::read(&request[..PADDED - 1]).is_err(),
            "an unpadded request is refused"
        );
    }

    #[test]
    fn frames_round_trip_and_fill_a_datagram() {
        let mut p = PayloadWriter::new(1, 2, Acks { latest: 3, bits: 4 });
        let full = [9u8; FRAGMENT_SIZE];
        let big = Piece {
            message: 5,
            index: 0,
            count: 2,
            bytes: &full,
        };
        let small = Piece {
            message: 6,
            index: 0,
            count: 1,
            bytes: b"hi",
        };
        assert!(p.fits(FRAGMENT_SIZE));
        p.frame(0, &big);
        p.frame(2, &small);
        assert!(
            !p.fits(FRAGMENT_SIZE),
            "a second full fragment does not fit"
        );
        let bytes = p.finish();
        assert!(bytes.len() <= MAX_DATAGRAM);

        let Ok(Packet::Payload { frames: body, .. }) = Packet::read(&bytes) else {
            panic!("a payload");
        };
        let read: Vec<_> = frames(body).collect::<Result<_, _>>().expect("parses");
        assert_eq!(
            read,
            vec![
                Frame {
                    channel: 0,
                    piece: big
                },
                Frame {
                    channel: 2,
                    piece: small
                }
            ]
        );
    }

    #[test]
    fn random_bytes_never_panic() {
        let mut rng = crate::rng::Rng::new(11);
        for _ in 0..50_000 {
            let len = (rng.next_u64() % 200) as usize;
            let mut bytes: Vec<u8> = (0..len).map(|_| rng.next_u64() as u8).collect();
            if let Some(kind) = bytes.first_mut() {
                *kind = (*kind % 8).max(1);
            }
            if let Ok(Packet::Payload { frames: body, .. }) = Packet::read(&bytes) {
                for frame in frames(body) {
                    if frame.is_err() {
                        break;
                    }
                }
            }
        }
    }
}
