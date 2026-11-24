//! The dialling side, natively: what a desktop client uses to reach a server
//! that only a browser would otherwise talk to, and what tests use.

use std::collections::VecDeque;
use std::net::{SocketAddr, TcpStream};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use tungstenite::WebSocket;

use super::{config, prepare, pump, write, Pumped, HANDSHAKE_TIMEOUT};
use crate::transport::{check_size, Datagram, Transport, TransportError};
use crate::Address;

/// The most datagrams a client holds before the engine drains them.
const MAX_INBOX: usize = 8192;

type Opened = Result<WebSocket<TcpStream>, String>;

enum State {
    /// The TCP connect and the handshake, on a thread of their own: std has
    /// no non-blocking connect, and a game's frame must not wait on one.
    Opening(Receiver<Opened>),
    Open(Box<WebSocket<TcpStream>>),
    Closed,
}

/// A WebSocket to one server. Sends before it is open are dropped, as a
/// datagram to a host still booting would be; the connection layer's
/// handshake retries until it is.
pub struct WebSocketClient {
    server: SocketAddr,
    state: State,
    inbox: VecDeque<Datagram>,
    closed: bool,
}

impl WebSocketClient {
    /// Starts connecting to the WebSocket server at `server`, at the path
    /// `/`. Returns at once; [`is_open`](Self::is_open) says when it is.
    pub fn connect(server: SocketAddr) -> Self {
        let (done, opened) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("voltra-net websocket connect".into())
            .spawn(move || {
                // The receiver is dropped if the client was; nothing to tell.
                let _ = done.send(open(server));
            });
        let state = match spawned {
            Ok(_) => State::Opening(opened),
            Err(e) => {
                log::warn!("could not start connecting to ws://{server}: {e}");
                State::Closed
            }
        };
        Self {
            server,
            state,
            inbox: VecDeque::new(),
            closed: false,
        }
    }

    /// The server's address, as datagrams from it are marked.
    pub fn server(&self) -> Address {
        Address::WebSocket(self.server)
    }

    /// Whether the stream is up and datagrams flow.
    pub fn is_open(&self) -> bool {
        matches!(self.state, State::Open(_))
    }

    /// Whether the stream has ended (or never opened) for good.
    pub fn is_closed(&self) -> bool {
        matches!(self.state, State::Closed)
    }

    fn end(&mut self) {
        if !matches!(self.state, State::Closed) {
            self.state = State::Closed;
            self.closed = true;
        }
    }
}

/// Connects and shakes hands, blocking: run on the opening thread.
fn open(server: SocketAddr) -> Opened {
    let stream = TcpStream::connect_timeout(&server, HANDSHAKE_TIMEOUT)
        .map_err(|e| format!("could not connect: {e}"))?;
    stream
        .set_read_timeout(Some(HANDSHAKE_TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT)))
        .map_err(|e| format!("could not set timeouts: {e}"))?;
    let (socket, _response) =
        tungstenite::client::client_with_config(format!("ws://{server}/"), stream, Some(config()))
            .map_err(|e| format!("the handshake failed: {e}"))?;
    socket
        .get_ref()
        .set_read_timeout(None)
        .and_then(|()| prepare(socket.get_ref()))
        .map_err(|e| format!("could not prepare the stream: {e}"))?;
    Ok(socket)
}

impl Transport for WebSocketClient {
    fn update(&mut self, _now: Duration) -> Result<(), TransportError> {
        if let State::Opening(opened) = &self.state {
            match opened.try_recv() {
                Ok(Ok(socket)) => {
                    log::debug!("websocket open to ws://{}", self.server);
                    self.state = State::Open(Box::new(socket));
                }
                Ok(Err(e)) => {
                    log::warn!("websocket to ws://{} did not open: {e}", self.server);
                    self.end();
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.end(),
            }
        }
        if let State::Open(socket) = &mut self.state {
            let room = MAX_INBOX.saturating_sub(self.inbox.len());
            let from = Address::WebSocket(self.server);
            if let Pumped::Closed = pump(socket, from, &mut self.inbox, room) {
                self.end();
            }
        }
        Ok(())
    }

    fn send(&mut self, to: Address, payload: &[u8]) -> Result<(), TransportError> {
        check_size(payload)?;
        if to != self.server() {
            return Err(TransportError::Unreachable(to));
        }
        match &mut self.state {
            State::Opening(_) => Ok(()),
            State::Open(socket) => {
                if write(socket, to, payload).is_err() {
                    self.end();
                }
                Ok(())
            }
            State::Closed => Err(TransportError::Closed),
        }
    }

    fn recv(&mut self) -> Result<Option<Datagram>, TransportError> {
        Ok(self.inbox.pop_front())
    }

    fn reaches(&self, to: Address) -> bool {
        to == self.server()
    }

    fn local_addr(&self) -> Option<Address> {
        match &self.state {
            State::Open(socket) => socket.get_ref().local_addr().ok().map(Address::WebSocket),
            _ => None,
        }
    }

    fn take_closed(&mut self) -> Option<Address> {
        std::mem::take(&mut self.closed).then(|| self.server())
    }

    fn close(&mut self, peer: Address) {
        if peer == self.server() {
            if let State::Open(socket) = &mut self.state {
                // Best effort, as on the server: the drop ends the stream.
                let _ = socket.close(None);
                let _ = socket.flush();
            }
            self.end();
            self.closed = false;
        }
    }
}
