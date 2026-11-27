//! The lobby and LAN discovery through `tick.net`: a listen server and a
//! client in a lobby over an in-memory network, getting ready and started
//! from their ticks; and a host found on the LAN (probed over loopback —
//! broadcast does not reach every container) by a client that can join it,
//! and listed as incompatible to one that could not, whose join is refused.

use std::cell::RefCell;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::rc::Rc;
use std::time::{Duration, Instant};

use voltra_core::net::{
    Advertise, Conditioned, DisconnectReason, HostOptions, LinkConditions, LobbyConfig, LobbyEvent,
    LobbyPhase, MemoryNetwork, NetConfig, NetEvent, SearchOptions,
};
use voltra_core::{App, Headless};

const FRAME: f32 = 1.0 / 60.0;

fn lobby(name: &str) -> LobbyConfig {
    LobbyConfig {
        name: name.into(),
        min_players: 2,
        ..LobbyConfig::default()
    }
}

type Seen = Rc<RefCell<Vec<LobbyEvent>>>;

/// A game that readies its player as soon as it is in the lobby, and —
/// hosting — starts the match once it can; every lobby event kept.
fn eager(config: LobbyConfig) -> (App, Seen) {
    let seen: Seen = Rc::default();
    let log = seen.clone();
    let app = App::default().with_lobby(config).with_update(move |tick| {
        log.borrow_mut()
            .extend(tick.net.lobby_events().iter().cloned());
        assert!(
            tick.net.received().is_empty(),
            "a lobby message reached the game: {:?}",
            tick.net.received()
        );
        let Some(lobby) = tick.net.lobby_mut() else {
            return;
        };
        if lobby.is_in() && !lobby.is_ready() && lobby.lobby().phase() == LobbyPhase::Waiting {
            lobby.set_ready(true).expect("in, waiting");
        }
        let startable = lobby
            .local_player()
            .is_some_and(|me| lobby.lobby().can_start(me).is_ok());
        if startable {
            lobby.start().expect("startable");
        }
    });
    (app, seen)
}

#[test]
fn a_listen_host_and_a_client_ready_up_and_start_through_their_ticks() {
    let network = MemoryNetwork::new();
    let link = LinkConditions {
        latency: Duration::from_millis(20),
        loss: 0.1,
        ..LinkConditions::default()
    };
    // The host's sends go straight out: its goodbye is not held in a
    // simulated delay its session's end would cut short. The guest's link
    // is the bad one, both ways of the handshake and the lobby feel it.
    let server_transport = Conditioned::new(network.endpoint(), LinkConditions::default(), 1);
    let address = server_transport.inner().address();

    let (app, on_host) = eager(lobby("Host"));
    let mut host = Headless::new(app);
    host.net_mut()
        .host_on(server_transport, NetConfig::default());
    if let Some(endpoint) = host.net_mut().endpoint_mut() {
        endpoint.set_host_plays(true);
    }
    let (app, on_guest) = eager(lobby("Guest\u{202E}"));
    let mut guest = Headless::new(app);
    guest.net_mut().join_on(
        Conditioned::new(network.endpoint(), link, 2),
        address,
        NetConfig::default(),
    );

    for _ in 0..600 {
        host.step(1, FRAME);
        guest.step(1, FRAME);
        let started = |seen: &Seen| seen.borrow().contains(&LobbyEvent::Started);
        if started(&on_host) && started(&on_guest) {
            break;
        }
    }

    for (end, seen) in [(&host, &on_host), (&guest, &on_guest)] {
        let lobby = end.net().lobby().expect("a lobby");
        assert_eq!(
            lobby.lobby().phase(),
            LobbyPhase::Playing,
            "{:?}",
            seen.borrow()
        );
        let names: Vec<&str> = lobby
            .lobby()
            .entries()
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(names, ["Host", "Guest"]);
        assert!(seen.borrow().contains(&LobbyEvent::Started));
    }
    assert!(host.net().lobby().is_some_and(|l| l.is_host()));

    // The host leaving ends the lobby for the guest.
    host.net_mut().stop();
    for _ in 0..120 {
        host.step(1, FRAME);
        guest.step(1, FRAME);
        if on_guest.borrow().contains(&LobbyEvent::Closed) {
            break;
        }
    }
    assert!(
        on_guest.borrow().contains(&LobbyEvent::Closed),
        "{:?}",
        on_guest.borrow()
    );
}

fn loopback() -> IpAddr {
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

#[test]
fn a_host_is_found_on_the_lan_and_one_of_another_build_is_listed_but_refused() {
    let config = NetConfig {
        protocol: 0xD15C,
        ..NetConfig::default()
    };
    let mut host = Headless::new(App::default().with_lobby(lobby("Host")));
    host.net_mut()
        .host(&HostOptions {
            bind: loopback(),
            udp_port: Some(0),
            websocket_port: None,
            listen: true,
            config: config.clone(),
            discovery: Some(Advertise {
                port: 0,
                name: "Ada's room".into(),
                game: "test-game".into(),
                ..Advertise::default()
            }),
        })
        .expect("hosts");
    let probe_at: SocketAddr = host.net().discovery_addr().expect("answering probes");

    // Each asks with the fingerprint of the session it would join with.
    let search = |game: &mut Headless| {
        let fingerprint = game.net().session_fingerprint(config.clone());
        game.net_mut()
            .discover(SearchOptions {
                broadcast: false,
                targets: vec![probe_at],
                timeout: Duration::from_secs(3),
                game: "test-game".into(),
                fingerprint,
                ..SearchOptions::default()
            })
            .expect("a socket");
    };
    // One that would join as the host does, with the lobby; one built
    // without it.
    let mut same = Headless::new(App::default().with_lobby(lobby("Same")));
    search(&mut same);
    let mut other = Headless::new(App::default());
    search(&mut other);

    let start = Instant::now();
    while (same.net().discovered().is_empty() || other.net().discovered().is_empty())
        && start.elapsed() < Duration::from_secs(5)
    {
        host.step(1, FRAME);
        same.step(1, FRAME);
        other.step(1, FRAME);
        std::thread::sleep(Duration::from_millis(5));
    }

    let found = same.net().discovered().to_vec();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].name, "Ada's room");
    assert!(found[0].compatible, "{found:?}");
    assert_eq!(
        (found[0].players, found[0].max_players),
        (1, 8),
        "the host is in"
    );
    let address = found[0].address().expect("joinable over UDP").to_owned();
    assert!(!other.net().discovered()[0].compatible);

    // Joining what discovery said: the one that matches gets in, the other
    // is refused at the door.
    same.net_mut()
        .join(&address, config.clone())
        .expect("an address");
    other.net_mut().join(&address, config).expect("an address");
    let mut refused = false;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        host.step(1, FRAME);
        same.step(1, FRAME);
        other.step(1, FRAME);
        refused |= other.net().events().iter().any(|e| {
            matches!(
                e,
                NetEvent::Disconnected {
                    reason: DisconnectReason::ProtocolMismatch,
                    ..
                }
            )
        });
        if refused && same.net().lobby().is_some_and(|l| l.is_in()) {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(refused, "the other build was let in");
    let names: Vec<String> = host
        .net()
        .lobby()
        .expect("a lobby")
        .lobby()
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .collect();
    assert_eq!(names, ["Host", "Same"]);
}

#[test]
fn with_a_lobby_the_match_is_replicated_to_its_players_only() {
    use voltra_core::net::{Endpoint, PeerId};
    use voltra_core::replication::ReplicationConfig;

    let network = MemoryNetwork::new();
    let transport = network.endpoint();
    let address = transport.address();
    let rules = LobbyConfig {
        min_players: 1,
        join_timeout: Duration::from_secs(2),
        ..lobby("Server")
    };
    let mut server = Headless::new(
        App::default()
            .with_lobby(rules.clone())
            .with_replication(ReplicationConfig::default()),
    );
    server.net_mut().host_on(transport, NetConfig::default());
    let session = server.net().endpoint().expect("hosting").config().clone();

    let (app, _) = eager(lobby("Player"));
    let mut player = Headless::new(app.with_replication(ReplicationConfig::default()));
    player
        .net_mut()
        .join_on(network.endpoint(), address, NetConfig::default());
    // Connected, the right session, and never a word to the lobby.
    let mut silent = Endpoint::client(network.endpoint(), address, session);

    let mut now = Duration::ZERO;
    let mut frame = |server: &mut Headless, player: &mut Headless, silent: &mut Endpoint| {
        now += Duration::from_secs_f32(FRAME);
        server.step(1, FRAME);
        player.step(1, FRAME);
        silent.receive(now);
        silent.flush(now);
    };
    for _ in 0..60 {
        frame(&mut server, &mut player, &mut silent);
    }
    let me = player.net().lobby().and_then(|l| l.local_player());
    let me: PeerId = me.expect("the player is in");
    assert!(silent.local_id().is_some(), "the silent peer is connected");
    let replicated: Vec<PeerId> = server
        .net()
        .replication()
        .and_then(|r| r.server())
        .expect("replicating")
        .clients()
        .collect();
    assert_eq!(
        replicated,
        [me],
        "only the lobby's player is sent the world"
    );

    // Past the join deadline the silent peer is cut off; the player stays.
    for _ in 0..150 {
        frame(&mut server, &mut player, &mut silent);
    }
    assert_eq!(silent.local_id(), None, "cut off at the join deadline");
    assert_eq!(server.net().endpoint().map(Endpoint::peer_count), Some(1));
    assert!(player.net().lobby().is_some_and(|l| l.is_in()));
}
