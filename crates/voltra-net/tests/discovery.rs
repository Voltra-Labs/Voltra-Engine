//! LAN discovery over loopback. A container's network often drops
//! broadcasts (and CI's may), so the searches here probe the responder
//! directly at `127.0.0.1` — the same packets a broadcast carries, on the
//! same socket a broadcast reaches; only the route differs.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use voltra_net::{
    Advert, DiscoveryLimits, DiscoveryResponder, LanSearch, SearchOptions, ServerInfo, PROBE_SIZE,
};

const LOOPBACK: Ipv4Addr = Ipv4Addr::LOCALHOST;

fn responder() -> DiscoveryResponder {
    DiscoveryResponder::bind(SocketAddr::from((LOOPBACK, 0))).expect("a socket")
}

fn advert(fingerprint: u64) -> Advert {
    Advert {
        name: "Ada's\n game".into(),
        game: "arena".into(),
        players: 1,
        max_players: 4,
        in_match: false,
        udp_port: Some(7777),
        websocket_port: Some(7778),
        fingerprint,
    }
}

fn search_for(responders: &[&DiscoveryResponder], game: &str, fingerprint: u64) -> LanSearch {
    LanSearch::start(SearchOptions {
        broadcast: false,
        targets: responders.iter().map(|r| r.local_socket()).collect(),
        game: game.to_owned(),
        fingerprint,
        timeout: Duration::from_secs(3),
        resend: Duration::from_millis(200),
        ..SearchOptions::default()
    })
    .expect("a socket")
}

/// Polls the search and the responders on the wall clock until the search
/// has `want` servers (or finishes), and answers what it found.
fn run(
    search: &mut LanSearch,
    responders: &mut [(&mut DiscoveryResponder, Advert)],
    want: usize,
) -> Vec<ServerInfo> {
    let start = Instant::now();
    while !search.is_finished() && search.servers().len() < want {
        let now = start.elapsed();
        search.poll(now);
        for (responder, advert) in responders.iter_mut() {
            responder.poll(now, advert);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    search.poll(start.elapsed());
    search.servers().to_vec()
}

#[test]
fn a_search_over_loopback_finds_a_server_and_says_how_to_join() {
    let mut server = responder();
    let mut search = search_for(&[&server], "arena", 0xF1);
    let found = run(&mut search, &mut [(&mut server, advert(0xF1))], 1);

    assert_eq!(found.len(), 1, "{found:?}");
    let info = &found[0];
    assert_eq!(info.name, "Ada's game", "sanitised for showing");
    assert_eq!(info.game, "arena");
    assert_eq!((info.players, info.max_players), (1, 4));
    assert!(info.compatible);
    assert!(info.has_room());
    assert_eq!(info.udp.as_deref(), Some("udp://127.0.0.1:7777"));
    assert_eq!(info.websocket.as_deref(), Some("ws://127.0.0.1:7778"));
    assert_eq!(info.address(), info.udp.as_deref());
    assert_eq!(server.stats().answered, 1);
}

#[test]
fn a_server_of_another_build_is_listed_as_incompatible_and_another_game_not_at_all() {
    let mut same = responder();
    let mut other_build = responder();
    let mut other_game = responder();
    let mut search = search_for(&[&same, &other_build, &other_game], "arena", 0xF1);
    let mut elsewhere = advert(0xF1);
    elsewhere.game = "platformer".into();
    let found = run(
        &mut search,
        &mut [
            (&mut same, advert(0xF1)),
            (&mut other_build, advert(0xF2)),
            (&mut other_game, elsewhere),
        ],
        // Run to the timeout: the other game must stay out of the list.
        usize::MAX,
    );

    assert_eq!(found.len(), 2, "{found:?}");
    let by = |at: SocketAddr| found.iter().find(|s| s.from == at).expect("listed");
    assert!(by(same.local_socket()).compatible);
    assert!(!by(other_build.local_socket()).compatible);
    assert!(other_game.stats().other_game >= 1);
    assert_eq!(other_game.stats().answered, 0);
}

#[test]
fn a_flood_of_probes_from_one_source_gets_a_trickle_of_answers_no_larger() {
    let mut server = DiscoveryResponder::with_limits(
        SocketAddr::from((LOOPBACK, 0)),
        DiscoveryLimits::default(),
    )
    .expect("a socket");
    let flood = UdpSocket::bind((LOOPBACK, 0)).expect("a socket");
    flood.set_nonblocking(true).expect("non-blocking");

    // A real probe, copied from a search's first send: what an attacker
    // replays from a forged source.
    let catcher = UdpSocket::bind((LOOPBACK, 0)).expect("a socket");
    catcher
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("a timeout");
    let mut search = LanSearch::start(SearchOptions {
        broadcast: false,
        targets: vec![catcher.local_addr().expect("bound")],
        ..SearchOptions::default()
    })
    .expect("a socket");
    search.poll(Duration::ZERO);
    let mut probe = [0u8; 512];
    let (len, _) = catcher.recv_from(&mut probe).expect("the probe");
    assert_eq!(len, PROBE_SIZE);

    for _ in 0..200 {
        flood
            .send_to(&probe[..len], server.local_socket())
            .expect("sent");
    }
    // Within one simulated second the source has its burst (4) and two a
    // second after it.
    for tick in 0..=60 {
        server.poll(Duration::from_millis(tick * 1000 / 60), &advert(1));
        std::thread::sleep(Duration::from_millis(1));
    }
    let stats = server.stats();
    assert!(
        (4..=6).contains(&stats.answered),
        "a burst and a trickle: {stats:?}"
    );
    assert_eq!(stats.answered + stats.limited, 200, "{stats:?}");

    let mut answer = [0u8; 512];
    let mut largest = 0;
    while let Ok((len, _)) = flood.recv_from(&mut answer) {
        largest = largest.max(len);
    }
    assert!(largest > 0 && largest <= PROBE_SIZE, "{largest}");
}

#[test]
fn junk_is_dropped_unanswered_and_a_real_probe_still_gets_through() {
    let mut server = responder();
    let junk = UdpSocket::bind((LOOPBACK, 0)).expect("a socket");
    let mut state = 0x2545_F491_u64;
    for len in [
        0usize,
        1,
        6,
        PROBE_SIZE - 1,
        PROBE_SIZE,
        PROBE_SIZE + 1,
        1200,
    ] {
        let bytes: Vec<u8> = (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect();
        junk.send_to(&bytes, server.local_socket()).expect("sent");
    }
    std::thread::sleep(Duration::from_millis(20));
    server.poll(Duration::ZERO, &advert(1));
    assert_eq!(server.stats().malformed, 7, "{:?}", server.stats());
    assert_eq!(server.stats().answered, 0);

    let mut search = search_for(&[&server], "", 1);
    let found = run(&mut search, &mut [(&mut server, advert(1))], 1);
    assert_eq!(found.len(), 1, "an empty game id asks every game");
}

#[test]
fn a_search_with_nobody_there_finishes_empty_at_its_timeout() {
    let mut search = LanSearch::start(SearchOptions {
        broadcast: false,
        targets: vec![SocketAddr::from((LOOPBACK, 9))],
        timeout: Duration::from_millis(300),
        ..SearchOptions::default()
    })
    .expect("a socket");
    search.poll(Duration::ZERO);
    assert!(!search.is_finished());
    search.poll(Duration::from_millis(299));
    assert!(!search.is_finished());
    search.poll(Duration::from_millis(300));
    assert!(search.is_finished());
    assert!(search.servers().is_empty());
}
