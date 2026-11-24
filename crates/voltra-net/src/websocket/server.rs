//! The listening side: accepts WebSocket connections without blocking.

use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;

use tungstenite::handshake::server::{NoCallback, ServerHandshake};
use tungstenite::handshake::{HandshakeError, MidHandshake};
use tungstenite::WebSocket;

use super::{config, prepare, pump, write, Pumped, HANDSHAKE_TIMEOUT};
use crate::transport::{check_size, Datagram, Transport, TransportError};
use crate::Address;

type Handshake = MidHandshake<ServerHandshake<TcpStream, NoCallback>>;

/// What a listener holds against untrusted connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebSocketLimits {
    /// Open streams at once; a connection past it is closed on accept.
    pub max_streams: usize,
    /// Handshakes in progress at once; past it, new connections are closed
    /// until one finishes. A client that opens TCP and never speaks costs a
    /// slot for at most [`handshake_timeout`](Self::handshake_timeout).
    pub max_handshakes: usize,
    /// How long a handshake may take before its connection is dropped.
    pub handshake_timeout: Duration,
    /// Datagrams waiting to be received, over every stream; a stream is not
    /// read past it until the engine drains some. TCP then pushes back on
    /// the sender rather than us buffering for it.
    pub max_inbox: usize,
}

impl Default for WebSocketLimits {
    fn default() -> Self {
        Self {
            max_streams: 1024,
            max_handshakes: 64,
            handshake_timeout: HANDSHAKE_TIMEOUT,
            max_inbox: 8192,
        }
    }
}

/// A non-blocking WebSocket listener: every accepted stream is a peer at
/// [`Address::WebSocket`] of its remote socket, and every binary message
/// on it a datagram.
pub struct WebSocketServer {
    listener: TcpListener,
    local: SocketAddr,
    limits: WebSocketLimits,
    now: Duration,
    handshakes: Vec<(Handshake, SocketAddr, Duration)>,
    streams: HashMap<SocketAddr, WebSocket<TcpStream>>,
    inbox: VecDeque<Datagram>,
    closed: VecDeque<Address>,
}

impl WebSocketServer {
    /// Listens at `addr` — `"0.0.0.0:7778"`, or port `0` for any free one —
    /// with the default limits.
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        Self::bind_with(addr, WebSocketLimits::default())
    }

    /// Listens at `addr` with `limits`.
    pub fn bind_with(addr: impl ToSocketAddrs, limits: WebSocketLimits) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let local = listener.local_addr()?;
        Ok(Self {
            listener,
            local,
            limits,
            now: Duration::ZERO,
            handshakes: Vec::new(),
            streams: HashMap::new(),
            inbox: VecDeque::new(),
            closed: VecDeque::new(),
        })
    }

    /// The socket address listened at — with the real port when `0` was
    /// asked for.
    pub fn local_socket(&self) -> SocketAddr {
        self.local
    }

    /// Streams open now, handshakes done.
    pub fn stream_count(&self) -> usize {
        self.streams.len()
    }

    fn accept(&mut self) {
        loop {
            let (stream, from) = match self.listener.accept() {
                Ok(accepted) => accepted,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                Err(e) => {
                    // A connection reset between the SYN and the accept, or
                    // out of descriptors: the next tick tries again.
                    log::debug!("websocket accept failed: {e}");
                    return;
                }
            };
            if self.handshakes.len() >= self.limits.max_handshakes
                || self.streams.len() + self.handshakes.len() >= self.limits.max_streams
            {
                log::warn!("refused a websocket connection from {from}: at the limit");
                continue;
            }
            if let Err(e) = prepare(&stream) {
                log::debug!("could not prepare the stream from {from}: {e}");
                continue;
            }
            match tungstenite::accept_with_config(stream, Some(config())) {
                Ok(socket) => self.open(from, socket),
                Err(HandshakeError::Interrupted(mid)) => {
                    self.handshakes.push((mid, from, self.now));
                }
                Err(HandshakeError::Failure(e)) => {
                    log::debug!("websocket handshake from {from} failed: {e}");
                }
            }
        }
    }

    fn finish_handshakes(&mut self) {
        for (mid, from, started) in std::mem::take(&mut self.handshakes) {
            if self.now.saturating_sub(started) > self.limits.handshake_timeout {
                log::debug!("websocket handshake from {from} timed out");
                continue;
            }
            match mid.handshake() {
                Ok(socket) => self.open(from, socket),
                Err(HandshakeError::Interrupted(mid)) => self.handshakes.push((mid, from, started)),
                Err(HandshakeError::Failure(e)) => {
                    log::debug!("websocket handshake from {from} failed: {e}");
                }
            }
        }
    }

    fn open(&mut self, from: SocketAddr, socket: WebSocket<TcpStream>) {
        log::debug!("websocket open from {from}");
        // A second stream from the same socket address means the first is
        // gone (the port was reused); the new one replaces it.
        if self.streams.insert(from, socket).is_some() {
            self.closed.push_back(Address::WebSocket(from));
        }
    }
}

impl Transport for WebSocketServer {
    fn update(&mut self, now: Duration) -> Result<(), TransportError> {
        self.now = now;
        self.accept();
        self.finish_handshakes();
        let mut ended = Vec::new();
        for (&from, socket) in &mut self.streams {
            let room = self.limits.max_inbox.saturating_sub(self.inbox.len());
            if let Pumped::Closed = pump(socket, Address::WebSocket(from), &mut self.inbox, room) {
                ended.push(from);
            }
        }
        for from in ended {
            self.streams.remove(&from);
            self.closed.push_back(Address::WebSocket(from));
        }
        Ok(())
    }

    fn send(&mut self, to: Address, payload: &[u8]) -> Result<(), TransportError> {
        check_size(payload)?;
        let Address::WebSocket(addr) = to else {
            return Err(TransportError::Unreachable(to));
        };
        let Some(socket) = self.streams.get_mut(&addr) else {
            return Err(TransportError::Unreachable(to));
        };
        if write(socket, to, payload).is_err() {
            self.streams.remove(&addr);
            self.closed.push_back(to);
        }
        Ok(())
    }

    fn recv(&mut self) -> Result<Option<Datagram>, TransportError> {
        Ok(self.inbox.pop_front())
    }

    fn reaches(&self, to: Address) -> bool {
        matches!(to, Address::WebSocket(_))
    }

    fn local_addr(&self) -> Option<Address> {
        Some(Address::WebSocket(self.local))
    }

    fn take_closed(&mut self) -> Option<Address> {
        self.closed.pop_front()
    }

    fn close(&mut self, peer: Address) {
        if let Address::WebSocket(addr) = peer {
            if let Some(mut socket) = self.streams.remove(&addr) {
                // Best effort: the close frame is queued and flushed if the
                // stream takes it now; the drop closes the TCP stream anyway.
                let _ = socket.close(None);
                let _ = socket.flush();
            }
        }
    }
}
