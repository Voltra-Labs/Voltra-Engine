//! Networking: datagram transports, and (above them) connections.
//!
//! The only crate that opens a socket. Everything is polled once a tick
//! from the engine's own loop — no thread is required and no async runtime
//! is pulled in, natively or in a browser.
//!
//! - [`Transport`]: datagrams in and out, non-blocking. Implemented by
//!   [`UdpTransport`], the native [`WebSocketServer`] and
//!   [`WebSocketClient`], the browser's `BrowserWebSocket`, the in-process
//!   [`MemoryNetwork`], and two wrappers: [`Conditioned`] (simulated loss,
//!   latency, jitter, duplication) and [`MultiTransport`] (several at once).
//!
//! - [`Endpoint`]: a server or a client over a transport — a handshake with
//!   a protocol version and the game's fingerprint, keep-alives, timeouts,
//!   [`DisconnectReason`]s; [`ChannelKind`]s reliable-ordered,
//!   reliable-unordered and unreliable-sequenced; messages split into
//!   fragments and put back together; typed messages through [`encode`] and
//!   [`Message::decode`] (serde over `postcard`).
//!
//! Everything that arrives is untrusted: a transport bounds a datagram to
//! [`MAX_DATAGRAM`] bytes before anyone sees it, and the endpoint reads every
//! packet with bounds-checked lengths, takes message ids only inside the
//! window an honest sender can be in, and charges every buffer to a budget
//! before allocating it. A peer that breaks those bounds is disconnected
//! with [`DisconnectReason::Violation`]; nothing it sends can panic us.

#![warn(missing_docs)]

mod acks;
mod address;
mod channel;
mod conditions;
mod config;
mod connection;
mod endpoint;
mod event;
mod memory;
mod message;
mod multi;
mod packet;
mod rng;
mod sequence;
mod transport;
mod wire;

#[cfg(not(target_arch = "wasm32"))]
mod udp;
#[cfg(not(target_arch = "wasm32"))]
mod websocket;

#[cfg(target_arch = "wasm32")]
mod browser;

pub use address::Address;
pub use channel::{
    ChannelId, ChannelKind, DEFAULT_CHANNELS, FRAGMENT_SIZE, MAX_CHANNELS, RELIABLE_WINDOW,
};
pub use conditions::{ConditionStats, Conditioned, LinkConditions};
pub use config::{NetConfig, MAX_MESSAGE_LIMIT, PROTOCOL_VERSION};
pub use connection::ConnectionStats;
pub use endpoint::{Endpoint, SendError};
pub use event::{DisconnectReason, NetEvent, PeerId};
pub use memory::{MemoryNetwork, MemoryTransport};
pub use message::{decode, encode, CodecError, Message};
pub use multi::MultiTransport;
pub use transport::{Datagram, Transport, TransportError, MAX_DATAGRAM};

#[cfg(target_arch = "wasm32")]
pub use browser::BrowserWebSocket;
#[cfg(not(target_arch = "wasm32"))]
pub use udp::UdpTransport;
#[cfg(not(target_arch = "wasm32"))]
pub use websocket::{WebSocketClient, WebSocketLimits, WebSocketServer};
