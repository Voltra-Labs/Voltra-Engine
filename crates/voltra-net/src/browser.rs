//! The browser's WebSocket, behind the same polled trait as a socket.
//!
//! A page is only ever called back: the browser runs `onmessage` between
//! frames, and the handler pushes into a queue the engine drains on its own
//! tick, which keeps the trait's polled shape with no executor at all.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{BinaryType, CloseEvent, MessageEvent, WebSocket};

use crate::transport::{check_size, Datagram, Transport, TransportError, MAX_DATAGRAM};
use crate::Address;

/// The most datagrams held between ticks; past it new ones are dropped.
const MAX_INBOX: usize = 8192;

/// The most bytes the browser may be holding for the server before further
/// sends are dropped — a stalled connection must not grow the page's
/// memory without end.
const MAX_BUFFERED: u32 = 256 * 1024;

#[derive(Default)]
struct Shared {
    inbox: VecDeque<Datagram>,
    closed: bool,
    reported: bool,
}

/// A WebSocket a page opened to a game server, whose datagrams come from
/// [`Address::Server`].
pub struct BrowserWebSocket {
    socket: WebSocket,
    shared: Rc<RefCell<Shared>>,
    // Held for as long as the socket may call them.
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    _on_close: Closure<dyn FnMut(CloseEvent)>,
}

impl BrowserWebSocket {
    /// Opens a WebSocket to `url` (`ws://host:port/` or `wss://…`).
    /// Returns at once; sends before it is open are dropped, as on the
    /// native client.
    pub fn connect(url: &str) -> Result<Self, TransportError> {
        let socket = WebSocket::new(url).map_err(|e| {
            log::warn!("could not open a websocket to {url}: {e:?}");
            TransportError::Closed
        })?;
        socket.set_binary_type(BinaryType::Arraybuffer);
        let shared = Rc::new(RefCell::new(Shared::default()));

        let inbox = shared.clone();
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let Ok(buffer) = event.data().dyn_into::<js_sys::ArrayBuffer>() else {
                log::debug!("ignored a websocket message that was not binary");
                return;
            };
            let bytes = js_sys::Uint8Array::new(&buffer);
            let len = bytes.length() as usize;
            let mut shared = inbox.borrow_mut();
            if len > MAX_DATAGRAM || shared.inbox.len() >= MAX_INBOX {
                log::debug!("dropped a websocket message of {len} bytes");
                return;
            }
            shared.inbox.push_back(Datagram {
                from: Address::Server,
                payload: bytes.to_vec(),
            });
        });
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

        let closed = shared.clone();
        let on_close = Closure::<dyn FnMut(CloseEvent)>::new(move |event: CloseEvent| {
            log::info!(
                "the websocket closed ({}{})",
                event.code(),
                if event.was_clean() {
                    ""
                } else {
                    ", not cleanly"
                }
            );
            closed.borrow_mut().closed = true;
        });
        socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));

        Ok(Self {
            socket,
            shared,
            _on_message: on_message,
            _on_close: on_close,
        })
    }

    /// Whether the socket is open and datagrams flow.
    pub fn is_open(&self) -> bool {
        self.socket.ready_state() == WebSocket::OPEN
    }

    /// The server's address, as datagrams from it are marked.
    pub fn server(&self) -> Address {
        Address::Server
    }
}

impl Transport for BrowserWebSocket {
    fn send(&mut self, to: Address, payload: &[u8]) -> Result<(), TransportError> {
        check_size(payload)?;
        if to != Address::Server {
            return Err(TransportError::Unreachable(to));
        }
        match self.socket.ready_state() {
            WebSocket::CONNECTING => Ok(()),
            WebSocket::OPEN => {
                if self.socket.buffered_amount() > MAX_BUFFERED {
                    log::debug!("the websocket is not keeping up; dropped a datagram");
                    return Ok(());
                }
                self.socket.send_with_u8_array(payload).map_err(|e| {
                    log::warn!("websocket send failed: {e:?}");
                    TransportError::Closed
                })
            }
            _ => Err(TransportError::Closed),
        }
    }

    fn recv(&mut self) -> Result<Option<Datagram>, TransportError> {
        Ok(self.shared.borrow_mut().inbox.pop_front())
    }

    fn reaches(&self, to: Address) -> bool {
        to == Address::Server
    }

    fn local_addr(&self) -> Option<Address> {
        None
    }

    fn take_closed(&mut self) -> Option<Address> {
        let mut shared = self.shared.borrow_mut();
        (shared.closed && !std::mem::replace(&mut shared.reported, true)).then_some(Address::Server)
    }

    fn close(&mut self, peer: Address) {
        if peer == Address::Server {
            if let Err(e) = self.socket.close() {
                log::debug!("websocket close failed: {e:?}");
            }
            let mut shared = self.shared.borrow_mut();
            shared.closed = true;
            shared.reported = true;
        }
    }
}

impl Drop for BrowserWebSocket {
    fn drop(&mut self) {
        self.socket.set_onmessage(None);
        self.socket.set_onclose(None);
        if let Err(e) = self.socket.close() {
            log::debug!("websocket close failed: {e:?}");
        }
    }
}
