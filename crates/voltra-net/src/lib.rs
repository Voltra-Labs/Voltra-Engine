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
//! Everything that arrives is untrusted: a transport bounds a datagram to
//! [`MAX_DATAGRAM`] bytes before anyone sees it.

#![warn(missing_docs)]

mod address;
mod conditions;
mod memory;
mod multi;
mod rng;
mod transport;

#[cfg(not(target_arch = "wasm32"))]
mod udp;
#[cfg(not(target_arch = "wasm32"))]
mod websocket;

#[cfg(target_arch = "wasm32")]
mod browser;

pub use address::Address;
pub use conditions::{ConditionStats, Conditioned, LinkConditions};
pub use memory::{MemoryNetwork, MemoryTransport};
pub use multi::MultiTransport;
pub use transport::{Datagram, Transport, TransportError, MAX_DATAGRAM};

#[cfg(target_arch = "wasm32")]
pub use browser::BrowserWebSocket;
#[cfg(not(target_arch = "wasm32"))]
pub use udp::UdpTransport;
#[cfg(not(target_arch = "wasm32"))]
pub use websocket::{WebSocketClient, WebSocketLimits, WebSocketServer};
