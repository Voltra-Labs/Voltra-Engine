//! Peers that do not join the lobby the way a client does: one whose hello
//! and goodbye land in the same server receive, and one that connects and
//! never says hello. Neither may leave the server holding anything.

use std::time::Duration;

use voltra_net::{
    Address, DisconnectReason, Endpoint, LobbyConfig, LobbyEvent, LobbySession, MemoryNetwork,
    NetConfig, NetEvent, PeerId,
};

const TICK: Duration = Duration::from_millis(16);

fn rules() -> LobbyConfig {
    LobbyConfig {
        max_players: 2,
        min_players: 1,
        join_timeout: Duration::from_secs(1),
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
        let transport = network.endpoint();
        let address = transport.address();
        let endpoint = Endpoint::server(transport, rules().net_config(NetConfig::default()));
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
    let (mut server, address) = Server::new(&network);
    // A session of the right shape, with no lobby on it: it never asks in.
    let mut silent = Endpoint::client(
        network.endpoint(),
        address,
        rules().net_config(NetConfig::default()),
    );

    let mut now = Duration::ZERO;
    let mut connected_at = None;
    let mut cut_off = None;
    while now < Duration::from_secs(5) && cut_off.is_none() {
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
        held >= rules().join_timeout && held < rules().join_timeout + Duration::from_millis(500),
        "held its slot for {held:?}"
    );
    assert!(server.lobby.lobby().is_empty());
    assert_eq!(server.endpoint.peer_count(), 0, "the slot is free again");
}
