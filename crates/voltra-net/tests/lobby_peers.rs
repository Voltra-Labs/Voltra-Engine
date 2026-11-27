//! Peers that do not join the lobby the way a client does: one whose hello
//! and goodbye land in the same server receive, one that connects and
//! never says hello, and one whose hello an outage holds up. The first two
//! may not leave the server holding anything; the last is not cut off for
//! an outage its session survives.

use std::time::Duration;

use voltra_net::{
    Address, Conditioned, DisconnectReason, Endpoint, LinkConditions, LobbyConfig, LobbyEvent,
    LobbySession, MemoryNetwork, NetConfig, NetEvent, PeerId, JOIN_GRACE,
};

const TICK: Duration = Duration::from_millis(16);

fn rules() -> LobbyConfig {
    LobbyConfig {
        max_players: 2,
        min_players: 1,
        ..LobbyConfig::default()
    }
}

/// A dedicated server's endpoint and lobby on a clean in-memory link.
struct Server {
    endpoint: Endpoint,
    lobby: LobbySession,
    seen: Vec<LobbyEvent>,
    net: Vec<NetEvent>,
}

impl Server {
    fn new(network: &MemoryNetwork) -> (Self, Address) {
        Self::with(network, NetConfig::default())
    }

    /// A server whose session is made from `session`.
    fn with(network: &MemoryNetwork, session: NetConfig) -> (Self, Address) {
        let transport = network.endpoint();
        let address = transport.address();
        let endpoint = Endpoint::server(transport, rules().net_config(session));
        let lobby = LobbySession::new(rules(), &endpoint);
        let server = Self {
            endpoint,
            lobby,
            seen: Vec::new(),
            net: Vec::new(),
        };
        (server, address)
    }

    fn step(&mut self, now: Duration) {
        self.endpoint.receive(now);
        self.net.extend_from_slice(self.endpoint.events());
        self.lobby.receive(&mut self.endpoint, now);
        self.seen.extend_from_slice(self.lobby.events());
        self.lobby.send(&mut self.endpoint);
        self.endpoint.flush(now);
    }
}

#[test]
fn a_hello_and_a_goodbye_in_one_receive_leave_no_entry_behind() {
    let network = MemoryNetwork::new();
    let (mut server, address) = Server::new(&network);
    let config = rules().net_config(NetConfig::default());

    // Twice the lobby's slots, each one in and gone in a single server
    // frame: a ghost each time would fill it for good.
    for round in 0..4 {
        let mut endpoint = Endpoint::client(network.endpoint(), address, config.clone());
        let mut client = LobbySession::new(
            LobbyConfig {
                name: format!("Ghost {round}"),
                ..rules()
            },
            &endpoint,
        );
        let mut now = Duration::from_secs(round * 10);
        // Connected on both ends; the client's hello not yet sent.
        for _ in 0..20 {
            now += TICK;
            server.step(now);
            endpoint.receive(now);
            if endpoint.local_id().is_some() {
                break;
            }
            endpoint.flush(now);
        }
        let me = endpoint.local_id().expect("let in");
        // The hello goes out, and the goodbye right behind it: the server
        // reads both in its next receive.
        client.receive(&mut endpoint, now);
        client.send(&mut endpoint);
        endpoint.flush(now);
        endpoint.disconnect(PeerId::SERVER);
        endpoint.flush(now);

        now += TICK;
        server.step(now);
        assert!(
            server.net.iter().any(|event| matches!(
                event,
                NetEvent::Disconnected { peer, .. } if *peer == me
            )),
            "round {round}: the goodbye came in that receive"
        );
        for _ in 0..5 {
            now += TICK;
            server.step(now);
        }
        assert!(
            server.lobby.lobby().is_empty(),
            "round {round}: nobody is in a lobby everyone left: {:?}",
            server.lobby.lobby().entries()
        );
        assert_eq!(server.endpoint.peer_count(), 0);
    }
    assert!(
        !server
            .seen
            .iter()
            .any(|event| matches!(event, LobbyEvent::Joined { .. })),
        "a hello from a peer already gone lets nobody in: {:?}",
        server.seen
    );
}

#[test]
fn a_peer_that_never_says_hello_is_cut_off_at_the_join_deadline() {
    let network = MemoryNetwork::new();
    // A short session timeout, so the deadline it sets is short too.
    let session = NetConfig {
        timeout: Duration::from_secs(1),
        ..NetConfig::default()
    };
    let deadline = session.timeout + JOIN_GRACE;
    let (mut server, address) = Server::with(&network, session.clone());
    // A session of the right shape, with no lobby on it: it never asks in,
    // and its keep-alives hold the connection open.
    let mut silent = Endpoint::client(network.endpoint(), address, rules().net_config(session));

    let mut now = Duration::ZERO;
    let mut connected_at = None;
    let mut cut_off = None;
    while now < Duration::from_secs(8) && cut_off.is_none() {
        now += TICK;
        server.step(now);
        silent.receive(now);
        for event in silent.events() {
            match event {
                NetEvent::Connected(_) => connected_at = Some(now),
                NetEvent::Disconnected { reason, .. } => cut_off = Some((now, *reason)),
            }
        }
        silent.flush(now);
    }
    let connected_at = connected_at.expect("the session let it in");
    let (at, reason) = cut_off.expect("cut off");
    assert_eq!(reason, DisconnectReason::Kicked);
    let held = at - connected_at;
    assert!(
        held >= deadline && held < deadline + Duration::from_millis(500),
        "held its slot for {held:?}"
    );
    assert!(server.lobby.lobby().is_empty());
    assert_eq!(server.endpoint.peer_count(), 0, "the slot is free again");
}

#[test]
fn an_outage_the_session_survives_does_not_cost_the_join() {
    let network = MemoryNetwork::new();
    // The default session: dropped after 10 s of silence.
    let (mut server, address) = Server::new(&network);
    let transport = Conditioned::new(network.endpoint(), LinkConditions::default(), 3);
    let link = transport.control();
    let mut endpoint =
        Endpoint::client(transport, address, rules().net_config(NetConfig::default()));
    let mut client = LobbySession::new(
        LobbyConfig {
            name: "Ada".into(),
            ..rules()
        },
        &endpoint,
    );

    let mut now = Duration::ZERO;
    let mut outage_until = None;
    while now < Duration::from_secs(20) && !client.is_in() {
        now += TICK;
        server.step(now);
        endpoint.receive(now);
        client.receive(&mut endpoint, now);
        // Connected: the link goes dark before the hello is out, for 7 s —
        // longer than a 5 s deadline, shorter than the session's timeout.
        if outage_until.is_none() && endpoint.local_id().is_some() {
            link.set(LinkConditions {
                loss: 1.0,
                ..LinkConditions::default()
            });
            outage_until = Some(now + Duration::from_secs(7));
        }
        if outage_until.is_some_and(|until| now >= until) {
            link.set(LinkConditions::default());
        }
        client.send(&mut endpoint);
        endpoint.flush(now);
    }
    assert!(outage_until.is_some(), "connected");
    assert!(client.is_in(), "joined once the link came back");
    assert!(
        !server
            .net
            .iter()
            .any(|e| matches!(e, NetEvent::Disconnected { .. })),
        "never cut off: {:?}",
        server.net
    );
    assert_eq!(server.lobby.lobby().len(), 1);
}

#[test]
fn a_lobby_made_on_a_session_under_way_catches_up() {
    let network = MemoryNetwork::new();
    let transport = network.endpoint();
    let address = transport.address();
    let config = rules().net_config(NetConfig::default());
    let mut host = Endpoint::server(transport, config.clone());
    host.set_host_plays(true);
    let mut guest = Endpoint::client(network.endpoint(), address, config);

    // The session connects, its events read and gone, with no lobby yet.
    let mut now = Duration::ZERO;
    for _ in 0..30 {
        now += TICK;
        host.receive(now);
        host.flush(now);
        guest.receive(now);
        guest.flush(now);
    }
    assert!(guest.is_connected(PeerId::SERVER));

    let mut on_host = LobbySession::new(
        LobbyConfig {
            name: "Host".into(),
            ..rules()
        },
        &host,
    );
    let mut on_guest = LobbySession::new(
        LobbyConfig {
            name: "Guest".into(),
            ..rules()
        },
        &guest,
    );
    assert!(
        on_host.is_in() && on_host.is_host(),
        "the host's own player is in at once"
    );
    for _ in 0..30 {
        now += TICK;
        host.receive(now);
        on_host.receive(&mut host, now);
        on_host.send(&mut host);
        host.flush(now);
        guest.receive(now);
        on_guest.receive(&mut guest, now);
        on_guest.send(&mut guest);
        guest.flush(now);
    }
    let names: Vec<&str> = on_host
        .lobby()
        .entries()
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(names, ["Host", "Guest"], "the connected guest asked in");
    assert!(on_guest.is_in());
    assert_eq!(on_guest.lobby(), on_host.lobby());
}
