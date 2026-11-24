//! Hosting and joining by address: the transports a game needs, made from
//! a port or a URL, so a game says "host on 7777" or "join
//! udp://10.0.0.2:7777" and nothing about sockets.

use std::fmt;
use std::net::SocketAddr;

use crate::{Address, Endpoint, NetConfig};

/// The UDP port a host listens on unless told otherwise.
pub const DEFAULT_UDP_PORT: u16 = 7777;

/// The WebSocket port a host listens on unless told otherwise — one above
/// the UDP one.
pub const DEFAULT_WEBSOCKET_PORT: u16 = 7778;

/// How a game hosts a session.
#[derive(Debug, Clone, PartialEq)]
pub struct HostOptions {
    /// The interface to listen on: `0.0.0.0` for every one, `127.0.0.1` for
    /// this machine only.
    pub bind: std::net::IpAddr,
    /// The UDP port for native clients, `Some(0)` for any free one, `None`
    /// for no UDP.
    pub udp_port: Option<u16>,
    /// The WebSocket port for browser clients, `Some(0)` for any free one,
    /// `None` for none.
    pub websocket_port: Option<u16>,
    /// Whether the hosting process plays too — a listen server. Its own
    /// player is [`PeerId::SERVER`](crate::PeerId::SERVER), and joins the
    /// session the way a client does: a
    /// [`NetEvent::Connected`](crate::NetEvent::Connected) for it is the
    /// first event, so one code path spawns every player.
    pub listen: bool,
    /// The session's configuration; clients must agree with it.
    pub config: NetConfig,
}

impl Default for HostOptions {
    fn default() -> Self {
        Self {
            bind: std::net::IpAddr::from([0, 0, 0, 0]),
            udp_port: Some(DEFAULT_UDP_PORT),
            websocket_port: Some(DEFAULT_WEBSOCKET_PORT),
            listen: false,
            config: NetConfig::default(),
        }
    }
}

/// Why a game could not host or join.
#[derive(Debug)]
pub enum SessionError {
    /// A socket would not open: the port is taken, or not ours to use.
    Io(std::io::Error),
    /// Hosting with neither a UDP nor a WebSocket port.
    NoTransport,
    /// Not a server address this build can dial: `udp://host:port` or
    /// `ws://host:port` natively, `ws://` or `wss://` in a browser.
    BadAddress(String),
    /// Hosting from a browser, which cannot listen.
    Unsupported,
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::NoTransport => write!(f, "a host needs a UDP or a WebSocket port"),
            Self::BadAddress(address) => write!(
                f,
                "{address:?} is not a server address this build can reach \
                 (udp://host:port or ws://host:port)"
            ),
            Self::Unsupported => write!(f, "a browser cannot host a session"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<std::io::Error> for SessionError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Opens the sockets `options` name and hosts a session on them.
#[cfg(not(target_arch = "wasm32"))]
pub fn host(options: &HostOptions) -> Result<Endpoint, SessionError> {
    let mut transports = crate::MultiTransport::new();
    if let Some(port) = options.udp_port {
        let udp = crate::UdpTransport::bind(SocketAddr::new(options.bind, port))?;
        log::info!("hosting on udp://{}", udp.local_socket());
        transports = transports.with(udp);
    }
    if let Some(port) = options.websocket_port {
        let ws = crate::WebSocketServer::bind(SocketAddr::new(options.bind, port))?;
        log::info!("hosting on ws://{}", ws.local_socket());
        transports = transports.with(ws);
    }
    if transports.is_empty() {
        return Err(SessionError::NoTransport);
    }
    let mut endpoint = Endpoint::server(transports, options.config.clone());
    endpoint.set_host_plays(options.listen);
    Ok(endpoint)
}

/// A browser cannot listen: always [`SessionError::Unsupported`].
#[cfg(target_arch = "wasm32")]
pub fn host(_options: &HostOptions) -> Result<Endpoint, SessionError> {
    Err(SessionError::Unsupported)
}

/// Joins the server at `server` — `udp://host:port` or `ws://host:port`
/// natively, a `ws://` or `wss://` URL in a browser.
pub fn join(server: &str, config: NetConfig) -> Result<Endpoint, SessionError> {
    #[cfg(target_arch = "wasm32")]
    {
        if !(server.starts_with("ws://") || server.starts_with("wss://")) {
            return Err(SessionError::BadAddress(server.to_owned()));
        }
        let transport = crate::BrowserWebSocket::connect(server)
            .map_err(|_| SessionError::BadAddress(server.to_owned()))?;
        Ok(Endpoint::client(transport, Address::Server, config))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        match parse(server)? {
            Address::Udp(addr) => {
                let any = if addr.is_ipv4() {
                    "0.0.0.0:0"
                } else {
                    "[::]:0"
                };
                let udp = crate::UdpTransport::bind(any)?;
                Ok(Endpoint::client(udp, Address::Udp(addr), config))
            }
            Address::WebSocket(addr) => {
                let ws = crate::WebSocketClient::connect(addr);
                Ok(Endpoint::client(ws, Address::WebSocket(addr), config))
            }
            _ => Err(SessionError::BadAddress(server.to_owned())),
        }
    }
}

/// `udp://host:port` or `ws://host:port[/]` as the address it names; the
/// host is resolved here (a name may take a DNS lookup).
pub fn parse(server: &str) -> Result<Address, SessionError> {
    let bad = || SessionError::BadAddress(server.to_owned());
    let (scheme, rest) = server.split_once("://").ok_or_else(bad)?;
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    let addr = resolve(rest).ok_or_else(bad)?;
    match scheme {
        "udp" => Ok(Address::Udp(addr)),
        "ws" => Ok(Address::WebSocket(addr)),
        _ => Err(bad()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve(host_port: &str) -> Option<SocketAddr> {
    use std::net::ToSocketAddrs;
    host_port.to_socket_addrs().ok()?.next()
}

#[cfg(target_arch = "wasm32")]
fn resolve(host_port: &str) -> Option<SocketAddr> {
    host_port.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_addresses_parse_by_scheme() {
        assert_eq!(
            parse("udp://127.0.0.1:7777").expect("parses"),
            Address::Udp("127.0.0.1:7777".parse().expect("an address"))
        );
        assert_eq!(
            parse("ws://127.0.0.1:7778/").expect("parses"),
            Address::WebSocket("127.0.0.1:7778".parse().expect("an address"))
        );
        for bad in [
            "127.0.0.1:7777",
            "tcp://127.0.0.1:1",
            "udp://nowhere",
            "udp://",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}
