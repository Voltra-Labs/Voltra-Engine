//! Finding servers on the local network: a client broadcasts a probe, and
//! every server listening for one answers with its name, its players and
//! the fingerprint a client must match to join.
//!
//! Native only. A browser has no UDP, let alone broadcast: there
//! [`LanSearch::start`] and [`DiscoveryResponder::bind`] answer
//! [`DiscoveryError::Unsupported`], and a web client joins by address
//! (`ws://host:port`).
//!
//! Its own socket, on its own port ([`DEFAULT_DISCOVERY_PORT`], one above
//! the WebSocket port), not the session's: a probe is not a peer, and the
//! session's socket never answers a stranger with more than it sent.
//! Neither does this one — the probe is padded to [`PROBE_SIZE`] bytes and
//! an answer is at most [`MAX_ANSWER_SIZE`], smaller, so a forged source
//! address gets no amplification; answers are rate-limited per source and
//! in all ([`DiscoveryLimits`]); a datagram of any other size, another
//! magic, version or kind is dropped unread. Godot's and Unity's LAN
//! samples (and Minecraft's, Factorio's) have the same shape: a broadcast
//! probe on a known port, unicast answers.

// What only the native sockets use is dead code in a browser.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
mod limiter;
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
mod packet;
#[cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]
mod responder;
#[cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]
mod search;

pub use limiter::DiscoveryLimits;
pub use responder::{DiscoveryResponder, DiscoveryStats};
pub use search::LanSearch;

use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

use crate::NetConfig;

/// The UDP port servers listen for probes on unless told otherwise — one
/// above [`DEFAULT_WEBSOCKET_PORT`](crate::DEFAULT_WEBSOCKET_PORT), so not
/// the game's.
pub const DEFAULT_DISCOVERY_PORT: u16 = 7779;

/// The size of every probe, exactly: padded to it, so that no answer —
/// at most [`MAX_ANSWER_SIZE`] — is larger than what asked for it.
pub const PROBE_SIZE: usize = 192;

/// The largest answer: its fixed fields, the game id and the name at
/// their caps.
pub const MAX_ANSWER_SIZE: usize = packet::MAX_ANSWER;

/// The longest game id carried, in bytes; longer ones are cut.
pub const MAX_GAME_ID: usize = 32;

/// The longest server name carried, in bytes; longer ones are cut on a
/// character boundary.
pub const MAX_SERVER_NAME: usize = 64;

/// Whether and how a host answers probes: a field of
/// [`HostOptions`](crate::HostOptions), opened beside the session by
/// `tick.net.host` (or by [`advertise`] for a session made by hand).
#[derive(Debug, Clone, PartialEq)]
pub struct Advertise {
    /// The UDP port to listen for probes on — the same on every server of
    /// a LAN, so a client knows where to ask. `0` takes any free one (a
    /// test probing it directly).
    pub port: u16,
    /// The server's name in a client's list.
    pub name: String,
    /// The game id probes are matched against (a client asks for its own
    /// game's servers only). At most [`MAX_GAME_ID`] bytes.
    pub game: String,
    /// How often it answers.
    pub limits: DiscoveryLimits,
}

impl Default for Advertise {
    fn default() -> Self {
        Self {
            port: DEFAULT_DISCOVERY_PORT,
            name: "Voltra server".to_owned(),
            game: String::new(),
            limits: DiscoveryLimits::default(),
        }
    }
}

/// Opens the probe socket `options` ask for, on the interface the session
/// binds: `None` when they ask for none.
pub fn advertise(
    options: &crate::HostOptions,
) -> Result<Option<DiscoveryResponder>, DiscoveryError> {
    let Some(advertise) = &options.discovery else {
        return Ok(None);
    };
    let at = SocketAddr::new(options.bind, advertise.port);
    DiscoveryResponder::with_limits(at, advertise.limits).map(Some)
}

/// What a server says about itself to a probe.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Advert {
    /// What a player picks it by: "Ada's game".
    pub name: String,
    /// Which game it runs: probes asking for another are not answered. At
    /// most [`MAX_GAME_ID`] bytes.
    pub game: String,
    /// Players in it now.
    pub players: usize,
    /// Players it takes.
    pub max_players: usize,
    /// Whether a match is being played (a lobby's
    /// [`Playing`](crate::LobbyPhase::Playing)).
    pub in_match: bool,
    /// The UDP port native players join on, if it has one.
    pub udp_port: Option<u16>,
    /// The WebSocket port browsers join on, if it has one.
    pub websocket_port: Option<u16>,
    /// The session's [`NetConfig::fingerprint`]: a client with another is
    /// refused at the handshake, so lists the server as incompatible.
    pub fingerprint: u64,
}

/// A server a [`LanSearch`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    /// Its name, sanitised for showing.
    pub name: String,
    /// The game it runs.
    pub game: String,
    /// Players in it.
    pub players: usize,
    /// Players it takes.
    pub max_players: usize,
    /// Whether its match has started.
    pub in_match: bool,
    /// Whether this client could join it: the same wire protocol and the
    /// fingerprint the search asked for. One that is not would refuse the
    /// join as `ProtocolMismatch`; a list shows it greyed, or leaves it out.
    pub compatible: bool,
    /// Where its answer came from.
    pub from: SocketAddr,
    /// `udp://ip:port` to join it on, if it takes native players.
    pub udp: Option<String>,
    /// `ws://ip:port` to join it on, if it takes browsers.
    pub websocket: Option<String>,
}

impl ServerInfo {
    /// The address a native client joins it by: its UDP one, else its
    /// WebSocket one.
    pub fn address(&self) -> Option<&str> {
        self.udp.as_deref().or(self.websocket.as_deref())
    }

    /// Whether it has a free slot.
    pub fn has_room(&self) -> bool {
        self.players < self.max_players
    }
}

/// How a client looks for servers.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchOptions {
    /// The port servers listen for probes on.
    pub port: u16,
    /// How long the search listens for answers.
    pub timeout: Duration,
    /// How often the probe goes out again while it listens: a probe, or an
    /// answer, may be lost.
    pub resend: Duration,
    /// Whether to broadcast the probe to the whole local network
    /// (`255.255.255.255`). Off, only [`targets`](Self::targets) are asked.
    pub broadcast: bool,
    /// Addresses probed directly besides the broadcast: a subnet's own
    /// broadcast address, a machine by name, `127.0.0.1` — where broadcast
    /// does not reach (some containers and VPNs drop it).
    pub targets: Vec<SocketAddr>,
    /// Only servers of this game answer; empty for any game.
    pub game: String,
    /// The fingerprint a server must advertise to be
    /// [`compatible`](ServerInfo::compatible): the
    /// [`NetConfig::fingerprint`] of the config this client would join with.
    pub fingerprint: u64,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            port: DEFAULT_DISCOVERY_PORT,
            timeout: Duration::from_millis(1500),
            resend: Duration::from_millis(500),
            broadcast: true,
            targets: Vec::new(),
            game: String::new(),
            fingerprint: NetConfig::default().fingerprint(),
        }
    }
}

impl SearchOptions {
    /// A search for servers of `game` a client with `config` could join.
    pub fn for_game(game: &str, config: &NetConfig) -> Self {
        Self {
            game: game.to_owned(),
            fingerprint: config.fingerprint(),
            ..Self::default()
        }
    }
}

/// Why discovery could not start.
#[derive(Debug)]
pub enum DiscoveryError {
    /// The socket would not open: the port is taken (another server on
    /// this machine listens for probes), or not ours to use.
    Io(std::io::Error),
    /// In a browser, which has no UDP: join by address instead.
    Unsupported,
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Unsupported => write!(
                f,
                "LAN discovery needs UDP broadcast, which a browser does not have; \
                 join by address"
            ),
        }
    }
}

impl std::error::Error for DiscoveryError {}

impl From<std::io::Error> for DiscoveryError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
