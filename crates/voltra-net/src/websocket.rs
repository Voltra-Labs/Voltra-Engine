//! Native WebSockets: the listener a browser connects to, and a client.
//!
//! A browser cannot open UDP, so a server that wants web players listens
//! for WebSockets too — each binary message one datagram. Over TCP nothing
//! is lost or reordered, which makes the connection layer's acks redundant
//! there but harmless: the same protocol runs over both, unchanged.
//!
//! `tungstenite` drives the protocol over a `TcpStream` set non-blocking —
//! its handshakes resume where they stopped — so no thread or runtime is
//! needed on the listening side. Every limit an untrusted peer could push
//! against is set: the message and frame size (a datagram), the write
//! buffer, the number and age of half-open handshakes.

use std::io;
use std::net::TcpStream;
use std::time::Duration;

use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Message, WebSocket};

use crate::transport::{Datagram, MAX_DATAGRAM};
use crate::Address;

mod client;
mod server;

pub use client::WebSocketClient;
pub use server::{WebSocketLimits, WebSocketServer};

/// The most bytes one stream may have waiting to be written before further
/// datagrams to it are dropped: a peer that stops reading cannot make us
/// buffer without end.
const MAX_WRITE_BUFFER: usize = 256 * 1024;

/// The protocol limits both ends set: nothing larger than a datagram is
/// accepted, and writes go out as they are made.
fn config() -> WebSocketConfig {
    WebSocketConfig::default()
        .read_buffer_size(16 * 1024)
        // Eager: a datagram is written when sent, not batched behind the next.
        .write_buffer_size(0)
        .max_write_buffer_size(MAX_WRITE_BUFFER)
        .max_message_size(Some(MAX_DATAGRAM))
        .max_frame_size(Some(MAX_DATAGRAM))
}

/// What one pass over a stream found.
enum Pumped {
    /// Still open; whatever arrived is in the inbox.
    Open,
    /// Closed — by the peer, by an error, or by breaking a limit.
    Closed,
}

/// Reads what `socket` has into `inbox` as datagrams from `from`, at most
/// `budget` of them, and flushes what is waiting to be written.
fn pump(
    socket: &mut WebSocket<TcpStream>,
    from: Address,
    inbox: &mut std::collections::VecDeque<Datagram>,
    budget: usize,
) -> Pumped {
    for _ in 0..budget {
        match socket.read() {
            Ok(Message::Binary(bytes)) => inbox.push_back(Datagram {
                from,
                payload: bytes.to_vec(),
            }),
            // Pings are answered by tungstenite on the next write; a text
            // frame is not something our protocol sends.
            Ok(Message::Text(_)) => log::debug!("ignored a text message from {from}"),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) => {
                log::debug!("websocket to {from} ended: {e}");
                return Pumped::Closed;
            }
        }
    }
    match socket.flush() {
        Ok(()) => Pumped::Open,
        Err(tungstenite::Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => Pumped::Open,
        Err(e) => {
            log::debug!("websocket to {from} ended: {e}");
            Pumped::Closed
        }
    }
}

/// Sends one datagram on `socket`; a full buffer or a stream mid-flush
/// drops or queues it. `Err` only when the stream is finished.
fn write(socket: &mut WebSocket<TcpStream>, to: Address, payload: &[u8]) -> Result<(), ()> {
    match socket.send(Message::binary(payload.to_vec())) {
        Ok(()) => Ok(()),
        // Buffered by tungstenite; the next pump flushes it.
        Err(tungstenite::Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
        Err(tungstenite::Error::WriteBufferFull(_)) => {
            log::debug!("websocket to {to} is not keeping up; dropped a datagram");
            Ok(())
        }
        Err(e) => {
            log::debug!("websocket to {to} ended: {e}");
            Err(())
        }
    }
}

/// Prepares a freshly made TCP stream for a game's traffic: non-blocking,
/// and no Nagle delay holding a small datagram back.
fn prepare(stream: &TcpStream) -> io::Result<()> {
    stream.set_nodelay(true)?;
    stream.set_nonblocking(true)
}

/// How long a half-open handshake, either side, may take.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
