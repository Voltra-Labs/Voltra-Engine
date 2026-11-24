//! The one seam every socket sits behind: datagrams in, datagrams out.
//!
//! Everything above — connections, channels, replication — speaks only this
//! trait, so a test runs it over [`MemoryNetwork`](crate::MemoryNetwork), the
//! editor (stage 117) over a [`Conditioned`](crate::Conditioned) wrapper, and a
//! shipped game over UDP or a WebSocket, without knowing which.

use std::fmt;
use std::io;
use std::time::Duration;

use crate::Address;

/// The largest datagram any transport carries, in bytes.
///
/// 1200 is what QUIC and netcode settle on: under the IPv6 minimum MTU of
/// 1280 with room for the IP and UDP headers, so a datagram is never
/// fragmented by a router on the way, where one lost piece loses the lot.
/// Bigger messages are split by the connection layer, not the network.
pub const MAX_DATAGRAM: usize = 1200;

/// One datagram, and who sent it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Datagram {
    /// The sender, as [`Transport::send`] would reach it back.
    pub from: Address,
    /// What arrived. Never longer than [`MAX_DATAGRAM`]: a transport drops a
    /// longer one before it is handed out.
    pub payload: Vec<u8>,
}

/// Why a transport could not do what it was asked.
#[derive(Debug)]
pub enum TransportError {
    /// The datagram was longer than [`MAX_DATAGRAM`]; nothing was sent.
    TooLarge(usize),
    /// This transport has no way to reach the address: another kind of address,
    /// or a stream peer that is not (or no longer) connected.
    Unreachable(Address),
    /// The transport itself has ended — its socket closed, its stream gone —
    /// and will carry nothing more.
    Closed,
    /// The operating system refused.
    Io(io::Error),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge(len) => write!(
                f,
                "a datagram of {len} bytes is over the limit of {MAX_DATAGRAM}"
            ),
            Self::Unreachable(to) => write!(f, "{to} cannot be reached through this transport"),
            Self::Closed => write!(f, "the transport is closed"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for TransportError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// A way to move datagrams between this process and others.
///
/// **Non-blocking, polled.** Nothing here waits: the engine calls
/// [`update`](Self::update) once a tick, drains [`recv`](Self::recv) until it
/// answers `None`, and sends what the tick produced. No thread is required
/// and no async runtime is involved — the shape of Godot's
/// `PacketPeer.poll()` and of renet's `update`/`send_packets`, and what a
/// browser, which only ever calls us back, can also be.
///
/// **Unreliable by contract**, as UDP is. A datagram may be lost,
/// duplicated or reordered, and a transport that cannot send one right now
/// (a full buffer, a stream still opening) drops it and answers `Ok`. An
/// error means the *request* was wrong or the transport is broken, never
/// "the network lost it". Reliability is the connection layer's job.
///
/// **Untrusted in**: what [`recv`](Self::recv) hands out came off a wire.
/// A transport bounds it to [`MAX_DATAGRAM`]; its *contents* are for the
/// layer above to validate.
pub trait Transport {
    /// The I/O that is neither a send nor a receive: accepting a stream,
    /// finishing a handshake, flushing what is buffered, releasing a delayed
    /// datagram. Once per tick, before draining [`recv`](Self::recv). `now`
    /// is the caller's clock — any origin, but never going backwards.
    fn update(&mut self, now: Duration) -> Result<(), TransportError> {
        let _ = now;
        Ok(())
    }

    /// Sends `payload` to `to`, or drops it (see the trait's contract).
    fn send(&mut self, to: Address, payload: &[u8]) -> Result<(), TransportError>;

    /// The next datagram that arrived, or `None` once there is none left
    /// this tick.
    fn recv(&mut self) -> Result<Option<Datagram>, TransportError>;

    /// Whether `to` is the kind of address this transport sends to — how a
    /// [`MultiTransport`](crate::MultiTransport) routes a send.
    fn reaches(&self, to: Address) -> bool;

    /// The address peers reach this end at, when it has one to give.
    fn local_addr(&self) -> Option<Address>;

    /// A stream peer whose connection ended since this was last asked.
    /// Datagram transports have none and keep the default.
    fn take_closed(&mut self) -> Option<Address> {
        None
    }

    /// Ends the stream to `peer`, on a transport that keeps one per peer.
    /// Nothing to do on a datagram transport, which keeps the default.
    fn close(&mut self, peer: Address) {
        let _ = peer;
    }
}

impl<T: Transport + ?Sized> Transport for Box<T> {
    fn update(&mut self, now: Duration) -> Result<(), TransportError> {
        (**self).update(now)
    }

    fn send(&mut self, to: Address, payload: &[u8]) -> Result<(), TransportError> {
        (**self).send(to, payload)
    }

    fn recv(&mut self) -> Result<Option<Datagram>, TransportError> {
        (**self).recv()
    }

    fn reaches(&self, to: Address) -> bool {
        (**self).reaches(to)
    }

    fn local_addr(&self) -> Option<Address> {
        (**self).local_addr()
    }

    fn take_closed(&mut self) -> Option<Address> {
        (**self).take_closed()
    }

    fn close(&mut self, peer: Address) {
        (**self).close(peer)
    }
}

/// [`TransportError::TooLarge`] when `payload` is over the limit.
pub(crate) fn check_size(payload: &[u8]) -> Result<(), TransportError> {
    if payload.len() > MAX_DATAGRAM {
        Err(TransportError::TooLarge(payload.len()))
    } else {
        Ok(())
    }
}
