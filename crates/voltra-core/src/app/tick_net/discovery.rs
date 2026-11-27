//! LAN discovery through `tick.net`: a host answers probes beside its
//! session, a game looks for servers and lists what answered.

use voltra_net::{
    Address, Advert, Advertise, DiscoveryError, DiscoveryResponder, HostOptions, LanSearch,
    LobbyPhase, NetConfig, SearchOptions, ServerInfo,
};

use super::Net;

/// A host's probe socket and what it says.
pub(super) struct Advertising {
    responder: DiscoveryResponder,
    advertise: Advertise,
    /// The session's fingerprint, taken when it was opened: the session
    /// does not change under it (a new one drops it).
    fingerprint: u64,
}

impl Net {
    /// Looks for servers on the local network with `options`, from the next
    /// frame, replacing a search under way: the list is
    /// [`discovered`](Self::discovered), filled as servers answer, for
    /// [`SearchOptions::timeout`]. Native only — in a browser,
    /// [`DiscoveryError::Unsupported`]: join by address there.
    pub fn discover(&mut self, options: SearchOptions) -> Result<(), DiscoveryError> {
        self.search = None;
        self.search = Some(LanSearch::start(options)?);
        Ok(())
    }

    /// Looks for servers of `game` this game could join with `config` (the
    /// one it would [`join`](Self::join) with): the fingerprint they must
    /// advertise to be [`compatible`](ServerInfo::compatible) is that of
    /// the session it would make, replication and lobby included.
    pub fn discover_game(&mut self, game: &str, config: NetConfig) -> Result<(), DiscoveryError> {
        let config = self.session_config(config);
        self.discover(SearchOptions::for_game(game, &config))
    }

    /// The fingerprint of the session this game would make from `config`
    /// — replication's channels and the lobby's added when they are on —
    /// which a server must advertise for a search to call it
    /// [`compatible`](ServerInfo::compatible).
    pub fn session_fingerprint(&self, config: NetConfig) -> u64 {
        self.session_config(config).fingerprint()
    }

    /// The servers the last search found so far.
    pub fn discovered(&self) -> &[ServerInfo] {
        self.search.as_ref().map_or(&[], LanSearch::servers)
    }

    /// Whether a search is still listening.
    pub fn is_discovering(&self) -> bool {
        self.search
            .as_ref()
            .is_some_and(|search| !search.is_finished())
    }

    /// Answers probes for this session as `advertise` says, on `bind`'s
    /// interface: what [`host`](Self::host) does with
    /// [`HostOptions::discovery`], for a session started by hand
    /// ([`host_on`](Self::host_on)). Ends with the session.
    pub fn advertise(
        &mut self,
        advertise: &Advertise,
        bind: std::net::IpAddr,
    ) -> Result<(), DiscoveryError> {
        let Some(endpoint) = self.endpoint.as_ref().filter(|e| e.is_server()) else {
            log::warn!("only a hosting session can be advertised");
            return Ok(());
        };
        let fingerprint = endpoint.config().fingerprint();
        let options = HostOptions {
            bind,
            discovery: Some(advertise.clone()),
            ..HostOptions::default()
        };
        if let Some(responder) = voltra_net::advertise(&options)? {
            log::info!(
                "answering LAN probes on udp://{} as {:?}",
                responder.local_socket(),
                advertise.name
            );
            self.advertising = Some(Advertising {
                responder,
                advertise: advertise.clone(),
                fingerprint,
            });
        }
        Ok(())
    }

    /// Where this session answers probes, if it does.
    pub fn discovery_addr(&self) -> Option<std::net::SocketAddr> {
        self.advertising
            .as_ref()
            .map(|advertising| advertising.responder.local_socket())
    }

    /// [`host`](Self::host)'s part: the probe socket, if asked for. One
    /// that will not open — another server on this machine has the port —
    /// leaves the session hosted, off the lists, and says so.
    pub(super) fn advertise_host(&mut self, options: &HostOptions) {
        let Some(advertise) = &options.discovery else {
            return;
        };
        if let Err(error) = self.advertise(advertise, options.bind) {
            log::warn!(
                "hosting, but not answering LAN probes on port {}: {error}",
                advertise.port
            );
        }
    }

    /// Probes answered and answers read: after the session's receive, so an
    /// answer says how many are in now.
    pub(super) fn poll_discovery(&mut self) {
        let now = self.now;
        if let Some(search) = self.search.as_mut() {
            search.poll(now);
        }
        let (Some(advertising), Some(endpoint)) = (self.advertising.as_mut(), &self.endpoint)
        else {
            return;
        };
        let port = |want: fn(&Address) -> bool| {
            endpoint.local_addrs().into_iter().find_map(|address| {
                if !want(&address) {
                    return None;
                }
                match address {
                    Address::Udp(at) | Address::WebSocket(at) => Some(at.port()),
                    _ => None,
                }
            })
        };
        let (players, max_players, in_match) = match &self.lobby {
            Some(lobby) => (
                lobby.lobby().len(),
                lobby.lobby().max_players(),
                lobby.lobby().phase() == LobbyPhase::Playing,
            ),
            None => (
                endpoint.players().len(),
                endpoint.config().max_clients + usize::from(endpoint.host_plays()),
                false,
            ),
        };
        let advert = Advert {
            name: advertising.advertise.name.clone(),
            game: advertising.advertise.game.clone(),
            players,
            max_players,
            in_match,
            udp_port: port(|a| matches!(a, Address::Udp(_))),
            websocket_port: port(|a| matches!(a, Address::WebSocket(_))),
            fingerprint: advertising.fingerprint,
        };
        advertising.responder.poll(now, &advert);
    }
}
