//! The lobby over the in-memory network, on a lossy link: joining under a
//! name, ready flags, the host starting, a full lobby and a match under way
//! refusing with their reasons, the host passing on (dedicated) or the
//! lobby closing (listen), and a client of another lobby build refused at
//! the handshake.

use std::time::Duration;

use voltra_net::{
    Address, Conditioned, DisconnectReason, Endpoint, LateJoin, LinkConditions, LobbyConfig,
    LobbyError, LobbyEvent, LobbyPhase, LobbyRefusal, LobbySession, MemoryNetwork, NetConfig,
    NetEvent, PeerId, LOBBY_WIRE_VERSION,
};

const TICK: Duration = Duration::from_millis(16);

fn lossy() -> LinkConditions {
    LinkConditions {
        latency: Duration::from_millis(20),
        jitter: Duration::from_millis(20),
        loss: 0.1,
        duplicate: 0.02,
        bandwidth: 0,
    }
}

fn rules(max: usize, min: usize) -> LobbyConfig {
    LobbyConfig {
        max_players: max,
        min_players: min,
        ..LobbyConfig::default()
    }
}

fn session_config(lobby: &LobbyConfig) -> NetConfig {
    lobby.net_config(NetConfig {
        protocol: 0x10BB,
        ..NetConfig::default()
    })
}

/// One end: its endpoint and its lobby, and every lobby event it saw.
struct End {
    endpoint: Endpoint,
    lobby: LobbySession,
    seen: Vec<LobbyEvent>,
    net: Vec<NetEvent>,
}

impl End {
    fn new(endpoint: Endpoint, config: LobbyConfig) -> Self {
        let lobby = LobbySession::new(config, &endpoint);
        Self {
            endpoint,
            lobby,
            seen: Vec::new(),
            net: Vec::new(),
        }
    }

    fn step(&mut self, now: Duration) {
        self.endpoint.receive(now);
        self.net.extend_from_slice(self.endpoint.events());
        self.lobby.receive(&mut self.endpoint, now);
        self.seen.extend_from_slice(self.lobby.events());
        self.lobby.send(&mut self.endpoint);
        self.endpoint.flush(now);
    }

    fn names(&self) -> Vec<String> {
        self.lobby
            .lobby()
            .entries()
            .iter()
            .map(|entry| entry.name.clone())
            .collect()
    }
}

struct Sim {
    network: MemoryNetwork,
    address: Address,
    server: End,
    clients: Vec<End>,
    now: Duration,
    seed: u64,
}

impl Sim {
    fn new(lobby: LobbyConfig, listen: bool) -> Self {
        let network = MemoryNetwork::new();
        let transport = network.endpoint();
        let address = transport.address();
        let mut endpoint = Endpoint::server(
            Conditioned::new(transport, lossy(), 1),
            session_config(&lobby),
        );
        endpoint.set_host_plays(listen);
        Self {
            server: End::new(endpoint, lobby),
            network,
            address,
            clients: Vec::new(),
            now: Duration::ZERO,
            seed: 10,
        }
    }

    fn join_with(&mut self, name: &str, net: NetConfig) -> usize {
        self.seed += 1;
        let transport = Conditioned::new(self.network.endpoint(), lossy(), self.seed);
        let lobby = LobbyConfig {
            name: name.to_owned(),
            ..LobbyConfig::default()
        };
        self.clients.push(End::new(
            Endpoint::client(transport, self.address, net),
            lobby,
        ));
        self.clients.len() - 1
    }

    fn join(&mut self, name: &str) -> usize {
        let net = session_config(&LobbyConfig::default());
        self.join_with(name, net)
    }

    fn step(&mut self) {
        self.now += TICK;
        self.server.step(self.now);
        for client in &mut self.clients {
            client.step(self.now);
        }
    }

    /// Steps until `done`, or ten simulated seconds.
    fn until(&mut self, what: &str, mut done: impl FnMut(&Self) -> bool) {
        for _ in 0..625 {
            if done(self) {
                return;
            }
            self.step();
        }
        panic!(
            "{what}: never, in ten seconds\nserver: {:?}\nclients: {:?}",
            self.server.lobby.lobby(),
            self.clients
                .iter()
                .map(|c| c.lobby.lobby().clone())
                .collect::<Vec<_>>()
        );
    }

    /// Whether every end's copy of the lobby is the server's.
    fn agreed(&self) -> bool {
        self.clients
            .iter()
            .filter(|client| client.lobby.is_in())
            .all(|client| client.lobby.lobby() == self.server.lobby.lobby())
    }
}

#[test]
fn players_join_get_ready_and_the_host_starts() {
    let mut sim = Sim::new(rules(4, 2), false);
    let ada = sim.join("Ada");
    sim.until("Ada in", |s| s.clients[ada].lobby.is_in());
    let bob = sim.join("  Ada\n");
    sim.until("both in, and agreed", |s| {
        s.server.lobby.lobby().len() == 2 && s.clients[bob].lobby.is_in() && s.agreed()
    });

    // A dedicated server's host is the first to join; names are made
    // unique and safe.
    assert_eq!(sim.server.names(), ["Ada", "Ada 2"]);
    assert!(sim.clients[ada].lobby.is_host());
    assert!(!sim.clients[bob].lobby.is_host());
    assert_eq!(
        sim.server.lobby.local_player(),
        None,
        "a dedicated server plays no one"
    );

    // Not yet: nobody is ready, and only the host may.
    assert_eq!(
        sim.clients[ada].lobby.start(),
        Err(LobbyError::NotEveryoneReady)
    );
    sim.clients[ada].lobby.set_ready(true).expect("in");
    sim.clients[bob].lobby.set_ready(true).expect("in");
    sim.until("both ready", |s| {
        s.server.lobby.lobby().all_ready() && s.agreed()
    });
    assert_eq!(sim.clients[bob].lobby.start(), Err(LobbyError::NotHost));
    sim.clients[ada].lobby.start().expect("the host, all ready");
    sim.until("started everywhere", |s| {
        s.server.lobby.lobby().phase() == LobbyPhase::Playing && s.agreed()
    });

    for end in std::iter::once(&sim.server).chain(&sim.clients) {
        assert_eq!(
            end.seen
                .iter()
                .filter(|e| **e == LobbyEvent::Started)
                .count(),
            1,
            "{:?}",
            end.seen
        );
        // Every Joined came before Started.
        let started = end
            .seen
            .iter()
            .position(|e| *e == LobbyEvent::Started)
            .expect("started");
        let joined = end
            .seen
            .iter()
            .filter(|e| matches!(e, LobbyEvent::Joined { .. }))
            .count();
        assert!(joined >= 2, "{:?}", end.seen);
        assert!(
            end.seen[..started]
                .iter()
                .filter(|e| matches!(e, LobbyEvent::Joined { .. }))
                .count()
                >= 2
        );
    }
    assert!(sim.server.seen.contains(&LobbyEvent::ReadyChanged {
        peer: sim.clients[bob].endpoint.local_id().expect("in"),
        ready: true,
    }));

    // The server's game ends the match: back in the lobby, nobody ready.
    assert_eq!(sim.clients[ada].lobby.finish(), Err(LobbyError::NotServer));
    sim.server.lobby.finish().expect("the server's");
    sim.until("finished everywhere", |s| {
        s.clients.iter().all(|c| {
            c.lobby.lobby().phase() == LobbyPhase::Waiting && c.seen.contains(&LobbyEvent::Finished)
        })
    });
    assert!(!sim.clients[ada].lobby.is_ready());
}

#[test]
fn a_full_lobby_refuses_with_its_reason_and_the_refused_leave() {
    let mut sim = Sim::new(rules(2, 1), false);
    let a = sim.join("a");
    let b = sim.join("b");
    sim.until("two in", |s| s.server.lobby.lobby().len() == 2);
    let c = sim.join("c");
    sim.until("the third refused, and gone", |s| {
        s.clients[c].lobby.refusal().is_some()
            && !s.clients[c].endpoint.is_connected(PeerId::SERVER)
    });

    assert_eq!(sim.clients[c].lobby.refusal(), Some(LobbyRefusal::Full));
    assert!(sim.clients[c]
        .seen
        .contains(&LobbyEvent::Refused(LobbyRefusal::Full)));
    sim.until("the third told it is closed", |s| {
        s.clients[c].seen.contains(&LobbyEvent::Closed)
    });
    assert_eq!(
        sim.server.lobby.lobby().len(),
        2,
        "the two kept their slots"
    );
    assert!(sim.clients[a].lobby.is_in() && sim.clients[b].lobby.is_in());
}

#[test]
fn a_match_under_way_refuses_a_late_joiner_unless_late_joins_play() {
    for late_join in [LateJoin::Refuse, LateJoin::Play] {
        let mut sim = Sim::new(
            LobbyConfig {
                late_join,
                ..rules(4, 1)
            },
            false,
        );
        let a = sim.join("a");
        sim.until("in", |s| s.clients[a].lobby.is_in());
        sim.clients[a].lobby.set_ready(true).expect("in");
        sim.until("ready", |s| {
            s.server.lobby.lobby().all_ready() && s.agreed()
        });
        sim.clients[a].lobby.start().expect("host");
        sim.until("playing", |s| {
            s.server.lobby.lobby().phase() == LobbyPhase::Playing
        });

        let late = sim.join("late");
        match late_join {
            LateJoin::Refuse => {
                sim.until("refused", |s| s.clients[late].lobby.refusal().is_some());
                assert_eq!(
                    sim.clients[late].lobby.refusal(),
                    Some(LobbyRefusal::MatchUnderway)
                );
            }
            LateJoin::Play => {
                sim.until("in, playing", |s| {
                    s.clients[late].lobby.is_in()
                        && s.clients[late].lobby.lobby().phase() == LobbyPhase::Playing
                });
                let id = sim.clients[late].endpoint.local_id().expect("in");
                assert!(sim
                    .server
                    .seen
                    .iter()
                    .any(|e| matches!(e, LobbyEvent::Joined { peer, .. } if *peer == id)));
            }
        }
    }
}

#[test]
fn a_dedicated_servers_host_passes_on_when_they_leave() {
    let mut sim = Sim::new(rules(4, 1), false);
    let first = sim.join("first");
    sim.until("first in", |s| s.clients[first].lobby.is_in());
    let second = sim.join("second");
    sim.until("second in", |s| {
        s.clients[second].lobby.is_in() && s.agreed()
    });
    assert!(sim.clients[first].lobby.is_host());

    let leaving = sim.clients[first].endpoint.local_id().expect("in");
    sim.clients[first].endpoint.disconnect(PeerId::SERVER);
    sim.until("the second hosts", |s| s.clients[second].lobby.is_host());

    let second_id = sim.clients[second].endpoint.local_id();
    assert!(sim
        .server
        .seen
        .contains(&LobbyEvent::Left { peer: leaving }));
    assert!(sim
        .server
        .seen
        .contains(&LobbyEvent::HostChanged(second_id)));
    assert!(sim.clients[second]
        .seen
        .contains(&LobbyEvent::Left { peer: leaving }));
}

#[test]
fn a_listen_host_hosts_and_its_leaving_closes_the_lobby() {
    let mut sim = Sim::new(
        LobbyConfig {
            name: "Host".into(),
            ..rules(4, 2)
        },
        true,
    );
    let guest = sim.join("Host");
    sim.until("guest in", |s| s.clients[guest].lobby.is_in() && s.agreed());

    assert_eq!(sim.server.lobby.local_player(), Some(PeerId::SERVER));
    assert!(sim.server.lobby.is_host());
    assert_eq!(sim.clients[guest].names(), ["Host", "Host 2"]);
    assert!(!sim.clients[guest].lobby.is_host());

    // The host readies and starts from its own end.
    sim.server.lobby.set_ready(true).expect("the host is in");
    sim.clients[guest].lobby.set_ready(true).expect("in");
    sim.until("all ready", |s| s.server.lobby.lobby().all_ready());
    sim.server.lobby.start().expect("the host");
    sim.until("the guest playing", |s| {
        s.clients[guest].lobby.lobby().phase() == LobbyPhase::Playing
    });

    // The host leaving ends the session, and the lobby with it.
    sim.server.endpoint.close_all(DisconnectReason::Shutdown);
    sim.until("the guest's lobby closed", |s| {
        s.clients[guest].seen.contains(&LobbyEvent::Closed)
    });
    assert!(sim.clients[guest].lobby.lobby().is_empty());
    assert!(!sim.clients[guest].lobby.is_in());
}

#[test]
fn a_client_of_another_lobby_build_or_none_is_refused_at_the_handshake() {
    let mut sim = Sim::new(rules(4, 1), false);
    let other_build = sim.join_with(
        "old",
        NetConfig {
            lobby_version: LOBBY_WIRE_VERSION + 1,
            ..session_config(&LobbyConfig::default())
        },
    );
    let no_lobby = sim.join_with(
        "none",
        NetConfig {
            protocol: 0x10BB,
            ..NetConfig::default()
        },
    );
    sim.until("both refused", |s| {
        [other_build, no_lobby].iter().all(|&c| {
            s.clients[c].net.iter().any(|e| {
                matches!(
                    e,
                    NetEvent::Disconnected {
                        reason: DisconnectReason::ProtocolMismatch,
                        ..
                    }
                )
            })
        })
    });
    assert!(sim.server.lobby.lobby().is_empty());
}

#[test]
fn a_lobby_message_that_is_not_one_gets_the_sender_kicked() {
    let mut sim = Sim::new(rules(4, 1), false);
    let honest = sim.join("honest");
    let liar = sim.join("liar");
    sim.until("both in", |s| s.server.lobby.lobby().len() == 2);
    let channel = LobbyConfig::default().channel;
    sim.clients[liar]
        .endpoint
        .send(PeerId::SERVER, channel, &[0xFF, 0xFF, 0xFF])
        .expect("queued");
    sim.until("the liar cut off", |s| {
        s.server.lobby.lobby().len() == 1 && !s.clients[liar].endpoint.is_connected(PeerId::SERVER)
    });
    assert!(sim.clients[honest].lobby.is_in());
}
