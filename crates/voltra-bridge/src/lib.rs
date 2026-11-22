//! The editor bridge: how a tool drives a running editor.
//!
//! One JSON object per line over a TCP socket on the loopback interface: a
//! [`Request`] in, a [`Response`] out, matched by id. The editor runs a
//! [`BridgeServer`] — off unless asked for — and applies what arrives inside
//! its own frame, where an edit lands in its undo history; `voltra-mcp`
//! talks to it through a [`BridgeClient`]. Neither side knows MCP here: the
//! editor binary stays free of the protocol and its async runtime.

pub mod client;
pub mod profile;
pub mod protocol;
pub mod server;

pub use client::BridgeClient;
pub use protocol::{Command, Request, Response};
pub use server::{BridgeServer, Pending};

/// The port a bridge listens on unless told otherwise.
pub const DEFAULT_PORT: u16 = 47_810;
