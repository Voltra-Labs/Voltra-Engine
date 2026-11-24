//! Where a datagram goes: one address type for every transport.

use std::fmt;
use std::net::SocketAddr;

/// A peer, as the transport that reaches it names it.
///
/// The kind is part of the address: a UDP peer and a WebSocket peer at the
/// same `ip:port` are two different peers, and a
/// [`MultiTransport`](crate::MultiTransport) routes a send by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Address {
    /// A UDP socket.
    Udp(SocketAddr),
    /// The other end of a native WebSocket's TCP stream.
    WebSocket(SocketAddr),
    /// An endpoint of a [`MemoryNetwork`](crate::MemoryNetwork).
    Memory(u32),
    /// The server a browser page opened its WebSocket to. A page knows the
    /// URL it dialled, never the socket behind it, and has only the one.
    Server,
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Udp(addr) => write!(f, "udp://{addr}"),
            Self::WebSocket(addr) => write!(f, "ws://{addr}"),
            Self::Memory(id) => write!(f, "memory://{id}"),
            Self::Server => write!(f, "the page's server"),
        }
    }
}
