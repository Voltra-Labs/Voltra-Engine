//! A match end to end: the arena's dedicated server as its own process,
//! and two headless clients — bots — joining it over UDP and playing until
//! a round is won. Both clients must agree on the winner and the scores the
//! server decided, and the server must have simulated each one's inputs —
//! sent by interest management, which on the arena's floor sends them all.
//! One client's sends are slowed by 60 ms: its zaps land where it saw the
//! other player, the server rewinding its longer round trip and more.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use arena::{net_config, replication_config, Arena, Pilot, Round, Score, Session, FIRST_SCENE};
use voltra_assets::AssetPath;
use voltra_core::net::{Address, Conditioned, LinkConditions, UdpTransport};
use voltra_core::replication::NetOwner;
use voltra_core::{App, Headless, WindowConfig};

/// The server process, killed however the test ends.
struct Server {
    child: Child,
    log: PathBuf,
}

impl Server {
    fn log(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}

fn start_server() -> (Server, u16) {
    let dir = std::env::temp_dir().join(format!("arena-match-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a folder");
    let ports = dir.join("ports");
    let _ = std::fs::remove_file(&ports);
    let log = dir.join("server.log");
    let child = Command::new(env!("CARGO_BIN_EXE_arena-server"))
        .arg("--asset-root")
        .arg(assets())
        .args([
            "--udp",
            "0",
            "--ws",
            "off",
            "--seconds",
            "90",
            "--port-file",
        ])
        .arg(&ports)
        .env("RUST_LOG", "info")
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&log).expect("the log"))
        .spawn()
        .expect("the server starts");
    let server = Server { child, log };
    let start = Instant::now();
    loop {
        if let Ok(text) = std::fs::read_to_string(&ports) {
            if let Some(port) = text
                .lines()
                .find_map(|line| line.strip_prefix("udp "))
                .and_then(|p| p.trim().parse().ok())
            {
                return (server, port);
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "the server never wrote its port:\n{}",
            server.log()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn client(port: u16) -> Headless {
    let app = App::new(WindowConfig::default())
        .with_asset_root(assets())
        .with_replication(replication_config())
        .with_game(Arena::new(
            Session::Join(format!("udp://127.0.0.1:{port}")),
            Pilot::Bot,
        ));
    let scene = AssetPath::new(FIRST_SCENE).expect("a path");
    Headless::open(app, &scene).expect("the scene loads")
}

/// A client whose sends wait `latency` (a link conditions what goes out):
/// a round trip that much longer. Joined before the game's first turn, so
/// the game leaves the session be.
fn slow_client(port: u16, latency: Duration) -> Headless {
    let app = App::new(WindowConfig::default())
        .with_asset_root(assets())
        .with_replication(replication_config())
        .with_game(Arena::new(Session::Solo, Pilot::Bot));
    let scene = AssetPath::new(FIRST_SCENE).expect("a path");
    let mut game = Headless::open(app, &scene).expect("the scene loads");
    let socket = UdpTransport::bind("127.0.0.1:0").expect("a socket");
    let link = LinkConditions {
        latency,
        jitter: Duration::ZERO,
        loss: 0.0,
        duplicate: 0.0,
        bandwidth: 0,
    };
    let server = Address::Udp(([127, 0, 0, 1], port).into());
    game.net_mut()
        .join_on(Conditioned::new(socket, link, 7), server, net_config());
    game
}

/// The server's zap lines: who zapped, and how many ticks back.
fn zaps(log: &str) -> Vec<(String, f64)> {
    log.lines()
        .filter_map(|line| {
            let (_, rest) = line.split_once("arena: ")?;
            let (shooter, rest) = rest.split_once(" zaps ")?;
            let (_, rest) = rest.split_once(", ")?;
            let back = rest.split_once(" ticks back")?.0.parse().ok()?;
            Some((shooter.to_owned(), back))
        })
        .collect()
}

fn round(game: &Headless) -> Round {
    game.world()
        .query::<Round>()
        .map(|(_, round)| *round)
        .next()
        .expect("the round entity")
}

/// Each player's owner and score, as `game` sees them.
fn scores(game: &Headless) -> Vec<(u32, u32)> {
    let mut scores: Vec<(u32, u32)> = game
        .world()
        .query::<NetOwner>()
        .filter_map(|(entity, owner)| Some((owner.0 .0, game.world().get::<Score>(entity)?.0)))
        .collect();
    scores.sort_unstable();
    scores
}

#[test]
fn a_server_and_two_clients_play_a_round_to_its_winner() {
    let (server, port) = start_server();
    let mut clients = [slow_client(port, Duration::from_millis(60)), client(port)];

    // Stepped with the wall clock: the server keeps real time.
    let start = Instant::now();
    let mut last = Instant::now();
    let winner = loop {
        std::thread::sleep(Duration::from_millis(8));
        let delta = last.elapsed().as_secs_f32();
        last = Instant::now();
        for client in &mut clients {
            client.step(1, delta);
        }
        if let (Some(a), Some(b)) = (round(&clients[0]).winner, round(&clients[1]).winner) {
            assert_eq!(a, b, "both clients see one winner");
            break a;
        }
        assert!(
            start.elapsed() < Duration::from_secs(60),
            "no winner in a minute; scores {:?} / {:?}\nserver:\n{}",
            scores(&clients[0]),
            scores(&clients[1]),
            server.log()
        );
    };

    let ids: Vec<u32> = clients
        .iter()
        .map(|c| c.net().local_id().expect("joined").0)
        .collect();
    assert!(ids.contains(&winner), "a client won: {winner} of {ids:?}");
    // The scores both clients see are the server's: the winner at the
    // target, and the two the same.
    let seen = scores(&clients[0]);
    assert_eq!(seen.len(), 2, "two players: {seen:?}");
    assert!(
        seen.contains(&(winner, round(&clients[0]).target)),
        "{seen:?}"
    );
    // Allow the other client's copy a few frames to land the same.
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(8));
        for client in &mut clients {
            client.step(1, 1.0 / 60.0);
        }
    }
    assert_eq!(scores(&clients[1]), scores(&clients[0]));
    // Each client predicted its own player, and the server simulated the
    // inputs it sent: a second of them at least.
    for client in &clients {
        let acked = client
            .net()
            .replication()
            .and_then(|r| r.client())
            .map(|c| c.input_ack());
        assert!(acked > Some(60), "inputs simulated: {acked:?}");
    }
    assert!(
        server.log().contains("wins round 1"),
        "the server decided it:\n{}",
        server.log()
    );
    // Sent by interest: the grid ran, and covering the floor it sent each
    // client both players, the coin and the round.
    assert!(
        server.log().contains("within 20 units of its interest"),
        "interest management on:\n{}",
        server.log()
    );
    for client in &clients {
        let stats = client
            .net()
            .replication()
            .and_then(|r| r.client())
            .map(|c| c.stats())
            .expect("a client");
        assert_eq!(stats.entities, 4, "{stats:?}");
        assert_eq!(stats.out_of_interest, 0, "{stats:?}");
    }
    // The slowed client's zaps landed where it saw its target, rewound by
    // its longer round trip (60 ms more, four ticks at 60 a second) and its
    // interpolation delay: further back than the other client's, and never
    // past the 200 ms bound (12 ticks). An input made before its client
    // drew anything carries no moment, and is tested at the present.
    let log = server.log();
    let mean = |peer: &Headless| {
        let id = peer.net().local_id().expect("joined").to_string();
        let backs: Vec<f64> = zaps(&log)
            .into_iter()
            .filter(|(shooter, _)| *shooter == id)
            .map(|(_, back)| back)
            .collect();
        assert!(!backs.is_empty(), "{id} landed a zap:\n{log}");
        assert!(
            backs.iter().all(|back| *back <= 12.0 + 1e-3),
            "{id}'s zaps rewound {backs:?}"
        );
        (
            backs.iter().sum::<f64>() / backs.len() as f64,
            backs.iter().copied().fold(0.0, f64::max),
        )
    };
    let (slow, slowest) = mean(&clients[0]);
    let (fast, _) = mean(&clients[1]);
    assert!(
        slowest >= 4.0 && slow > fast + 1.0,
        "the slowed client is rewound further: {slow:.1} (up to {slowest:.1}) \
         against {fast:.1} ticks\n{log}"
    );
}
