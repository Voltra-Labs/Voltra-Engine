//! A server's side of discovery: a socket that answers probes.

use std::net::SocketAddr;
use std::time::Duration;

use super::limiter::{DiscoveryLimits, Limiter};
use super::packet::{Answer, Probe};
use super::{Advert, DiscoveryError, PROBE_SIZE};

/// The most datagrams one [`poll`](DiscoveryResponder::poll) reads: a flood
/// costs a bounded amount of work a tick, and the rest wait in (or fall
/// out of) the socket's buffer.
const MAX_READS_PER_POLL: usize = 64;

/// What a responder has done with what it was sent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiscoveryStats {
    /// Probes answered.
    pub answered: u64,
    /// Probes not answered for the rate limit.
    pub limited: u64,
    /// Probes for another game, not answered.
    pub other_game: u64,
    /// Datagrams that were no probe of ours: dropped unread.
    pub malformed: u64,
}

/// Answers LAN discovery probes on a UDP port of its own, polled once a
/// tick like the session beside it (`tick.net` does it for a game that
/// hosts with [`HostOptions::discovery`](crate::HostOptions::discovery)).
pub struct DiscoveryResponder {
    #[cfg(not(target_arch = "wasm32"))]
    socket: std::net::UdpSocket,
    local: SocketAddr,
    limiter: Limiter,
    stats: DiscoveryStats,
}

impl std::fmt::Debug for DiscoveryResponder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscoveryResponder")
            .field("local", &self.local)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl DiscoveryResponder {
    /// Listens for probes at `addr` — `0.0.0.0:7779` to hear broadcasts on
    /// every interface — with the default [`DiscoveryLimits`].
    pub fn bind(addr: SocketAddr) -> Result<Self, DiscoveryError> {
        Self::with_limits(addr, DiscoveryLimits::default())
    }

    /// Listens for probes at `addr`, answering within `limits`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_limits(addr: SocketAddr, limits: DiscoveryLimits) -> Result<Self, DiscoveryError> {
        let socket = std::net::UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        let local = socket.local_addr()?;
        Ok(Self {
            socket,
            local,
            limiter: Limiter::new(limits),
            stats: DiscoveryStats::default(),
        })
    }

    /// A browser cannot listen: always [`DiscoveryError::Unsupported`].
    #[cfg(target_arch = "wasm32")]
    pub fn with_limits(
        _addr: SocketAddr,
        _limits: DiscoveryLimits,
    ) -> Result<Self, DiscoveryError> {
        Err(DiscoveryError::Unsupported)
    }

    /// The address it listens at, with the real port when `0` was asked.
    pub fn local_socket(&self) -> SocketAddr {
        self.local
    }

    /// What it has answered and dropped so far.
    pub fn stats(&self) -> DiscoveryStats {
        self.stats
    }

    /// Answers the probes waiting with `advert`, within the rate limits;
    /// `now` is the caller's clock, as an endpoint's is.
    pub fn poll(&mut self, now: Duration, advert: &Advert) {
        self.poll_with(now, || advert.clone());
    }

    /// [`poll`](Self::poll), with the advert made only when a probe is to
    /// be answered: most ticks there is none.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn poll_with(&mut self, now: Duration, advert: impl FnOnce() -> Advert) {
        use std::io::ErrorKind;
        let mut make = Some(advert);
        let mut made: Option<Advert> = None;
        // One byte over a probe: a read that fills it was something else.
        let mut buffer = [0u8; PROBE_SIZE + 1];
        for _ in 0..MAX_READS_PER_POLL {
            let (len, from) = match self.socket.recv_from(&mut buffer) {
                Ok(read) => read,
                Err(e) if e.kind() == ErrorKind::WouldBlock => return,
                Err(e)
                    if matches!(
                        e.kind(),
                        ErrorKind::ConnectionReset
                            | ErrorKind::ConnectionRefused
                            | ErrorKind::Interrupted
                    ) =>
                {
                    continue
                }
                Err(e) => {
                    log::warn!("discovery stopped reading: {e}");
                    return;
                }
            };
            let Ok(probe) = Probe::decode(&buffer[..len.min(buffer.len())]) else {
                self.stats.malformed += 1;
                continue;
            };
            if let Some(make) = make.take() {
                made = Some(make());
            }
            let Some(advert) = made.as_ref() else {
                continue;
            };
            if !probe.game.is_empty() && probe.game != advert.game {
                self.stats.other_game += 1;
                continue;
            }
            if !self.limiter.allow(from.ip(), now) {
                self.stats.limited += 1;
                continue;
            }
            let answer = Answer::new(probe.nonce, advert).encode();
            match self.socket.send_to(&answer, from) {
                Ok(_) => self.stats.answered += 1,
                Err(e) if e.kind() == ErrorKind::WouldBlock => {}
                Err(e) => log::debug!("discovery could not answer {from}: {e}"),
            }
        }
    }

    /// Never made in a browser.
    #[cfg(target_arch = "wasm32")]
    pub fn poll_with(&mut self, _now: Duration, _advert: impl FnOnce() -> Advert) {}
}
