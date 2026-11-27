//! A dedicated server and two clients on the loopback — one over UDP, one
//! over a WebSocket as a browser would come — both join, one leaves, the
//! other is kicked, and the server sees each event; then the binary itself
//! is started, found on the LAN (probed over loopback) and joined.

use std::process::{Child, Command};
use std::time::{Duration, Instant};

use voltra_core::net::{
    self, Address, DisconnectReason, Endpoint, HostOptions, NetConfig, NetEvent, PeerId,
};
use voltra_core::App;
use voltra_server::{Server, ServerOptions};

fn loopback() -> ServerOptions {
    ServerOptions {
        host: HostOptions {
            bind: [127, 0, 0, 1].into(),
            udp_port: Some(0),
            websocket_port: Some(0),
            ..HostOptions::default()
        },
        tick_rate: 60,
        lobby: None,
    }
}

fn url(address: Address) -> String {
    match address {
        Address::Udp(at) => format!("udp://{at}"),
        Address::WebSocket(at) => format!("ws://{at}"),
        other => panic!("a server is not at {other}"),
    }
}

/// What each side saw.
#[derive(Default)]
struct Seen {
    server: Vec<NetEvent>,
    clients: Vec<(usize, NetEvent)>,
}

/// Ticks the server and pumps the clients until `done`, or five seconds.
fn until(
    server: &mut Server,
    clients: &mut [&mut Endpoint],
    seen: &mut Seen,
    mut done: impl FnMut(&Server) -> bool,
) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        server.tick();
        seen.server.extend(server.events().iter().copied());
        for (i, client) in clients.iter_mut().enumerate() {
            client.update(start.elapsed());
            seen.clients.extend(client.events().iter().map(|&e| (i, e)));
        }
        if done(server) {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("timed out; the server saw {:?}", seen.server);
}

#[test]
fn two_clients_join_one_leaves_one_is_kicked_and_the_server_sees_each() {
    let mut server = Server::new(App::default(), &loopback()).expect("hosts");
    let addresses = server.addresses();
    assert_eq!(addresses.len(), 2, "UDP and WebSockets: {addresses:?}");
    let mut native = net::join(&url(addresses[0]), NetConfig::default()).expect("dials");
    let mut web = net::join(&url(addresses[1]), NetConfig::default()).expect("dials");
    let mut seen = Seen::default();

    until(&mut server, &mut [&mut native, &mut web], &mut seen, |s| {
        s.clients().len() == 2
    });
    assert_eq!(server.clients(), vec![PeerId(1), PeerId(2)]);
    let mut joined: Vec<PeerId> = seen
        .server
        .iter()
        .filter_map(|e| match e {
            NetEvent::Connected(peer) => Some(*peer),
            _ => None,
        })
        .collect();
    joined.sort();
    assert_eq!(joined, vec![PeerId(1), PeerId(2)]);

    // The native client leaves; the web one is kicked.
    let leaver = native.local_id().expect("in");
    let kicked = web.local_id().expect("in");
    native.disconnect(PeerId::SERVER);
    server.kick(kicked);
    until(&mut server, &mut [&mut native, &mut web], &mut seen, |s| {
        s.clients().is_empty()
    });
    // The kick may still be on its way to the web client.
    for _ in 0..100 {
        if seen.clients.iter().filter(|(who, _)| *who == 1).count() == 2 {
            break;
        }
        until(&mut server, &mut [&mut native, &mut web], &mut seen, |_| {
            true
        });
    }

    assert!(seen.server.contains(&NetEvent::Disconnected {
        peer: leaver,
        reason: DisconnectReason::Closed
    }));
    assert!(seen.server.contains(&NetEvent::Disconnected {
        peer: kicked,
        reason: DisconnectReason::Kicked
    }));
    let of = |client: usize| -> Vec<NetEvent> {
        seen.clients
            .iter()
            .filter(|(who, _)| *who == client)
            .map(|(_, e)| *e)
            .collect()
    };
    let ended = |reason| NetEvent::Disconnected {
        peer: PeerId::SERVER,
        reason,
    };
    let connected = NetEvent::Connected(PeerId::SERVER);
    assert_eq!(of(0), vec![connected, ended(DisconnectReason::Left)]);
    assert_eq!(of(1), vec![connected, ended(DisconnectReason::Kicked)]);
    assert!(server.ticks() > 0);
}

/// Kills the child when the test ends, however it ends.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn the_binary_hosts_a_scene_and_a_client_joins_it() {
    let dir = voltra_testkit::Scratch::new("voltra-server-test");
    let ports = dir.join("ports");
    let assets = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");
    let _server = Running(
        Command::new(env!("CARGO_BIN_EXE_voltra-server"))
            .args([
                "scenes/sandbox.ron",
                "--asset-root",
                assets,
                "--bind",
                "127.0.0.1",
                "--discovery",
                "0",
                "--name",
                "Sandbox",
            ])
            .args(["--udp", "0", "--ws", "0", "--seconds", "30", "--port-file"])
            .arg(&ports)
            .spawn()
            .expect("the server starts"),
    );

    let start = Instant::now();
    let read = |kind: &str| {
        let text = std::fs::read_to_string(&ports).ok()?;
        text.lines()
            .find_map(|line| line.strip_prefix(kind))
            .and_then(|port| port.trim().parse::<u16>().ok())
    };
    // The file is written whole once the server listens: the probe port's
    // line is its last.
    while read("discovery ").is_none() && start.elapsed() < Duration::from_secs(20) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let port = read("udp ").expect("the server wrote its ports");
    let probe_port = read("discovery ").expect("the server answers probes");

    let mut search = net::LanSearch::start(net::SearchOptions {
        broadcast: false,
        targets: vec![([127, 0, 0, 1], probe_port).into()],
        timeout: Duration::from_secs(5),
        ..net::SearchOptions::default()
    })
    .expect("a socket");
    let start = Instant::now();
    while search.servers().is_empty() && !search.is_finished() {
        search.poll(start.elapsed());
        std::thread::sleep(Duration::from_millis(10));
    }
    let found = search
        .servers()
        .first()
        .expect("the server answered")
        .clone();
    assert_eq!(found.name, "Sandbox");
    assert!(
        found.compatible,
        "a client of no game's protocol: {found:?}"
    );
    assert_eq!(found.udp, Some(format!("udp://127.0.0.1:{port}")));

    let mut client =
        net::join(&format!("udp://127.0.0.1:{port}"), NetConfig::default()).expect("dials");
    let mut events = Vec::new();
    let start = Instant::now();
    while client.local_id().is_none() && start.elapsed() < Duration::from_secs(10) {
        client.update(start.elapsed());
        events.extend(client.events().iter().copied());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(events, vec![NetEvent::Connected(PeerId::SERVER)]);
    assert_eq!(client.local_id(), Some(PeerId(1)));
    let _ = std::fs::remove_dir_all(&dir);
}
