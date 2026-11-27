//! A client's side of discovery: probes out, answers in, for a while.

use std::time::Duration;

use super::{DiscoveryError, SearchOptions, ServerInfo};

/// The most servers one search lists: a LAN party, not the internet.
#[cfg(not(target_arch = "wasm32"))]
const MAX_SERVERS: usize = 64;

/// The most datagrams one [`poll`](LanSearch::poll) reads.
#[cfg(not(target_arch = "wasm32"))]
const MAX_READS_PER_POLL: usize = 64;

/// A search for servers on the local network: polled like the rest of
/// the crate, never blocking. It probes at once and every
/// [`resend`](SearchOptions::resend), and lists each server that answers
/// until [`timeout`](SearchOptions::timeout) has passed since the first
/// poll; then it is [finished](Self::is_finished), its list kept.
pub struct LanSearch {
    #[cfg(not(target_arch = "wasm32"))]
    socket: std::net::UdpSocket,
    options: SearchOptions,
    nonce: u64,
    started: Option<Duration>,
    last_probe: Option<Duration>,
    finished: bool,
    servers: Vec<ServerInfo>,
}

impl std::fmt::Debug for LanSearch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LanSearch")
            .field("options", &self.options)
            .field("finished", &self.finished)
            .field("servers", &self.servers)
            .finish_non_exhaustive()
    }
}

impl LanSearch {
    /// Opens a socket to probe from. Nothing is sent before the first
    /// [`poll`](Self::poll).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn start(options: SearchOptions) -> Result<Self, DiscoveryError> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        if options.broadcast {
            socket.set_broadcast(true)?;
        }
        Ok(Self {
            socket,
            options,
            nonce: fresh_nonce(),
            started: None,
            last_probe: None,
            finished: false,
            servers: Vec::new(),
        })
    }

    /// A browser has no UDP: always [`DiscoveryError::Unsupported`]. Join
    /// by address instead.
    #[cfg(target_arch = "wasm32")]
    pub fn start(_options: SearchOptions) -> Result<Self, DiscoveryError> {
        Err(DiscoveryError::Unsupported)
    }

    /// The servers that answered so far, in the order they first did.
    pub fn servers(&self) -> &[ServerInfo] {
        &self.servers
    }

    /// Whether the search has listened for its whole timeout.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// What it looks for.
    pub fn options(&self) -> &SearchOptions {
        &self.options
    }

    /// Sends the probe when it is due and reads the answers that came;
    /// `now` is the caller's clock.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn poll(&mut self, now: Duration) {
        if self.finished {
            return;
        }
        let started = *self.started.get_or_insert(now);
        if now.saturating_sub(started) >= self.options.timeout {
            self.finished = true;
            return;
        }
        let due = self
            .last_probe
            .is_none_or(|last| now.saturating_sub(last) >= self.options.resend);
        if due {
            self.last_probe = Some(now);
            self.probe();
        }
        self.read();
    }

    /// Never made in a browser.
    #[cfg(target_arch = "wasm32")]
    pub fn poll(&mut self, _now: Duration) {}

    #[cfg(not(target_arch = "wasm32"))]
    fn probe(&mut self) {
        use std::net::{Ipv4Addr, SocketAddr};
        let probe = super::packet::Probe {
            nonce: self.nonce,
            game: self.options.game.clone(),
        }
        .encode();
        let broadcast = self
            .options
            .broadcast
            .then_some(SocketAddr::from((Ipv4Addr::BROADCAST, self.options.port)));
        for to in broadcast.into_iter().chain(self.options.targets.clone()) {
            if let Err(e) = self.socket.send_to(&probe, to) {
                // A network with no broadcast route, a target of the other
                // address family: the rest are still asked.
                log::debug!("discovery could not probe {to}: {e}");
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read(&mut self) {
        use std::io::ErrorKind;
        let mut buffer = [0u8; super::MAX_ANSWER_SIZE + 1];
        for _ in 0..MAX_READS_PER_POLL {
            let (len, from) = match self.socket.recv_from(&mut buffer) {
                Ok(read) => read,
                Err(e) if e.kind() == ErrorKind::WouldBlock => return,
                // About some other datagram — an ICMP unreachable for a
                // probe, one too long for the buffer (Windows' 10040): the
                // search goes on.
                Err(e) if crate::udp::is_transient(&e) => continue,
                Err(e) => {
                    log::warn!("discovery stopped listening: {e}");
                    self.finished = true;
                    return;
                }
            };
            let Ok(answer) = super::packet::Answer::decode(&buffer[..len]) else {
                continue;
            };
            if answer.nonce != self.nonce {
                continue;
            }
            let info = self.server_info(answer, from);
            // One server is one place to join: its address and game ports,
            // not where its answer came from — so another socket's answer
            // for the same ports updates it rather than listing it twice,
            // and an answer for other ports never hides it.
            let same = |known: &ServerInfo| {
                known.from.ip() == info.from.ip()
                    && known.udp == info.udp
                    && known.websocket == info.websocket
            };
            match self.servers.iter().position(same) {
                Some(known) => self.servers[known] = info,
                None if self.servers.len() < MAX_SERVERS => self.servers.push(info),
                None => {}
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn server_info(&self, answer: super::packet::Answer, from: std::net::SocketAddr) -> ServerInfo {
        use crate::lobby::visible_text;
        let advert = answer.advert;
        let name = visible_text(&advert.name, super::MAX_SERVER_NAME);
        let at = |scheme: &str, port: Option<u16>| {
            port.map(|port| format!("{scheme}://{}", std::net::SocketAddr::new(from.ip(), port)))
        };
        ServerInfo {
            name: if name.is_empty() {
                from.to_string()
            } else {
                name
            },
            game: visible_text(&advert.game, super::MAX_GAME_ID),
            players: advert.players,
            max_players: advert.max_players,
            in_match: advert.in_match,
            compatible: answer.protocol_version == crate::PROTOCOL_VERSION
                && advert.fingerprint == self.options.fingerprint,
            from,
            udp: at("udp", advert.udp_port),
            websocket: at("ws", advert.websocket_port),
        }
    }
}

/// A number another search, here or in another process, is unlikely to
/// pick: an answer to someone else's probe is not taken for ours.
#[cfg(not(target_arch = "wasm32"))]
fn fresh_nonce() -> u64 {
    use std::hash::BuildHasher;
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    std::collections::hash_map::RandomState::new().hash_one((count, std::process::id(), since))
}
