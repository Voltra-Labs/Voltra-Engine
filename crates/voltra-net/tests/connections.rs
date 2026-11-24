//! Connections over the in-memory network, under simulated loss, latency,
//! jitter (which reorders) and duplication: the handshake, the channels'
//! promises, the ways a connection ends, and hostile bytes.

use std::collections::BTreeSet;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use voltra_net::{
    Address, ChannelId, ChannelKind, Conditioned, Datagram, DisconnectReason, Endpoint,
    LinkConditions, MemoryNetwork, MemoryTransport, NetConfig, NetEvent, PeerId, SendError,
    Transport, TransportError, FRAGMENT_SIZE,
};

const TICK: Duration = Duration::from_millis(16);

fn bad_link() -> LinkConditions {
    LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::from_millis(40),
        loss: 0.2,
        duplicate: 0.05,
        bandwidth: 0,
    }
}

fn config() -> NetConfig {
    NetConfig {
        protocol: 0xC0FFEE,
        ..NetConfig::default()
    }
}

/// A server and its clients on one memory network, stepped together.
struct Sim {
    network: MemoryNetwork,
    server_addr: Address,
    server: Endpoint,
    clients: Vec<Endpoint>,
    now: Duration,
    seed: u64,
}

impl Sim {
    fn new(link: LinkConditions, server_config: NetConfig) -> Self {
        let network = MemoryNetwork::new();
        let transport = network.endpoint();
        let server_addr = transport.address();
        Self {
            server: Endpoint::server(Conditioned::new(transport, link, 1), server_config),
            network,
            server_addr,
            clients: Vec::new(),
            now: Duration::ZERO,
            seed: 100,
        }
    }

    fn join(&mut self, link: LinkConditions, config: NetConfig) -> usize {
        self.seed += 1;
        let transport = Conditioned::new(self.network.endpoint(), link, self.seed);
        self.clients
            .push(Endpoint::client(transport, self.server_addr, config));
        self.clients.len() - 1
    }

    /// One tick on every endpoint: receive, then `between`, then flush.
    fn step_with(&mut self, mut between: impl FnMut(&mut Endpoint, Option<usize>)) {
        self.now += TICK;
        self.server.receive(self.now);
        between(&mut self.server, None);
        self.server.flush(self.now);
        for (i, client) in self.clients.iter_mut().enumerate() {
            client.receive(self.now);
            between(client, Some(i));
            client.flush(self.now);
        }
    }

    fn step(&mut self) -> Vec<(Option<usize>, NetEvent)> {
        let mut events = Vec::new();
        self.step_with(|endpoint, who| {
            events.extend(endpoint.events().iter().map(|&e| (who, e)));
        });
        events
    }

    /// Steps until every client is connected, or five simulated seconds.
    fn connect_all(&mut self) {
        for _ in 0..300 {
            self.step();
            if self.clients.iter().all(|c| c.local_id().is_some()) {
                return;
            }
        }
        panic!("the clients did not all connect");
    }
}

#[test]
fn a_client_connects_and_both_ends_see_it() {
    let mut sim = Sim::new(LinkConditions::default(), config());
    sim.join(LinkConditions::default(), config());

    let mut events = Vec::new();
    for _ in 0..10 {
        events.extend(sim.step());
    }

    assert!(events.contains(&(None, NetEvent::Connected(PeerId(1)))));
    assert!(events.contains(&(Some(0), NetEvent::Connected(PeerId::SERVER))));
    assert_eq!(sim.clients[0].local_id(), Some(PeerId(1)));
    assert_eq!(sim.server.peers().collect::<Vec<_>>(), vec![PeerId(1)]);
    assert!(sim.server.is_server() && !sim.clients[0].is_server());
}

/// A message `i` of a size that walks from empty to several fragments.
fn message(i: u32) -> Vec<u8> {
    let len = (i as usize * 37) % (FRAGMENT_SIZE * 5 + 300);
    let mut bytes = i.to_le_bytes().to_vec();
    bytes.extend((0..len).map(|k| (k as u32 ^ i) as u8));
    bytes
}

#[test]
fn every_reliable_message_arrives_once_and_in_order_where_asked() {
    const COUNT: u32 = 1500;
    let mut sim = Sim::new(bad_link(), config());
    sim.join(bad_link(), config());
    sim.connect_all();

    let mut ordered_sent = 0u32;
    let mut unordered_sent = 0u32;
    let mut ordered = Vec::new();
    let mut unordered = Vec::new();
    for _ in 0..20_000 {
        sim.step_with(|endpoint, who| match who {
            // The server sends ordered; the client, unordered.
            None => {
                while ordered_sent < COUNT {
                    match endpoint.send(
                        PeerId(1),
                        ChannelId::RELIABLE_ORDERED,
                        &message(ordered_sent),
                    ) {
                        Ok(()) => ordered_sent += 1,
                        Err(SendError::ChannelFull(_)) => break,
                        Err(e) => panic!("{e}"),
                    }
                }
                for m in endpoint.take_messages() {
                    assert_eq!(m.channel, ChannelId::RELIABLE_UNORDERED);
                    unordered.push(m.bytes);
                }
            }
            Some(_) => {
                while unordered_sent < COUNT {
                    match endpoint.send(
                        PeerId::SERVER,
                        ChannelId::RELIABLE_UNORDERED,
                        &message(unordered_sent),
                    ) {
                        Ok(()) => unordered_sent += 1,
                        Err(SendError::ChannelFull(_)) => break,
                        Err(e) => panic!("{e}"),
                    }
                }
                for m in endpoint.take_messages() {
                    assert_eq!(m.channel, ChannelId::RELIABLE_ORDERED);
                    ordered.push(m.bytes);
                }
            }
        });
        if ordered.len() == COUNT as usize && unordered.len() == COUNT as usize {
            break;
        }
    }

    let expected: Vec<Vec<u8>> = (0..COUNT).map(message).collect();
    assert_eq!(ordered.len(), COUNT as usize, "every ordered message, once");
    assert!(ordered == expected, "and in the order sent");
    assert_eq!(unordered.len(), COUNT as usize, "every unordered message");
    let unique: BTreeSet<_> = unordered.iter().cloned().collect();
    assert_eq!(unique.len(), COUNT as usize, "each exactly once");
    assert_eq!(unique, expected.into_iter().collect::<BTreeSet<_>>());
    let stats = sim.server.stats(PeerId(1)).expect("connected");
    assert!(stats.resent > 0, "the loss was real: {stats:?}");
    assert!(stats.rtt >= Duration::from_millis(60), "{stats:?}");
}

#[test]
fn a_sequenced_channel_never_goes_backwards() {
    let mut sim = Sim::new(bad_link(), config());
    sim.join(bad_link(), config());
    sim.connect_all();

    let mut got = Vec::new();
    for tick in 0..400u32 {
        sim.step_with(|endpoint, who| match who {
            None => {
                // A position every tick, some of them long enough to split.
                let mut bytes = tick.to_le_bytes().to_vec();
                bytes.resize(if tick % 7 == 0 { 3000 } else { 8 }, 1);
                endpoint
                    .send(PeerId(1), ChannelId::UNRELIABLE_SEQUENCED, &bytes)
                    .expect("queues");
            }
            Some(_) => {
                for m in endpoint.take_messages() {
                    got.push(u32::from_le_bytes([
                        m.bytes[0], m.bytes[1], m.bytes[2], m.bytes[3],
                    ]));
                }
            }
        });
    }

    assert!(
        got.windows(2).all(|w| w[0] < w[1]),
        "strictly newer each time"
    );
    assert!(got.len() > 200, "most arrive: {}", got.len());
    assert!(got.len() < 400, "and some were lost on the way");
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum Chat {
    Hello { name: String },
    Move(f32, f32),
}

#[test]
fn typed_messages_round_trip() {
    let mut sim = Sim::new(LinkConditions::default(), config());
    sim.join(LinkConditions::default(), config());
    sim.connect_all();

    sim.clients[0]
        .send_message(
            PeerId::SERVER,
            ChannelId::RELIABLE_ORDERED,
            &Chat::Hello { name: "ada".into() },
        )
        .expect("queues");
    sim.clients[0]
        .send_message(
            PeerId::SERVER,
            ChannelId::RELIABLE_ORDERED,
            &Chat::Move(1.5, -2.0),
        )
        .expect("queues");
    let mut got = Vec::new();
    for _ in 0..5 {
        sim.step_with(|endpoint, who| {
            if who.is_none() {
                got.extend(
                    endpoint
                        .messages()
                        .iter()
                        .map(|m| (m.from, m.decode::<Chat>())),
                );
            }
        });
    }

    assert_eq!(
        got,
        vec![
            (PeerId(1), Ok(Chat::Hello { name: "ada".into() })),
            (PeerId(1), Ok(Chat::Move(1.5, -2.0)))
        ]
    );
}

#[test]
fn the_largest_message_goes_and_a_longer_one_is_refused() {
    let small = NetConfig {
        max_message_size: 10_000,
        ..config()
    };
    let mut sim = Sim::new(bad_link(), small.clone());
    sim.join(bad_link(), small);
    sim.connect_all();

    let biggest: Vec<u8> = (0..10_000u32).map(|i| (i * 7) as u8).collect();
    let refused = sim.clients[0].send(PeerId::SERVER, ChannelId::RELIABLE_ORDERED, &[0; 10_001]);
    assert_eq!(
        refused,
        Err(SendError::TooLarge {
            len: 10_001,
            max: 10_000
        })
    );
    sim.clients[0]
        .send(PeerId::SERVER, ChannelId::RELIABLE_ORDERED, &biggest)
        .expect("queues");
    let mut got = None;
    for _ in 0..300 {
        sim.step_with(|endpoint, who| {
            if who.is_none() {
                if let Some(m) = endpoint.take_messages().pop() {
                    got = Some(m.bytes);
                }
            }
        });
    }
    assert!(got == Some(biggest), "it arrived whole");
    assert_eq!(
        sim.clients[0].send(PeerId::SERVER, ChannelId(9), b"x"),
        Err(SendError::NoChannel(ChannelId(9)))
    );
    assert_eq!(
        sim.server
            .send(PeerId(7), ChannelId::RELIABLE_ORDERED, b"x"),
        Err(SendError::NotConnected(PeerId(7)))
    );
}

/// The events a client saw over `ticks` more ticks.
fn client_events(sim: &mut Sim, ticks: usize) -> Vec<(Option<usize>, NetEvent)> {
    (0..ticks).flat_map(|_| sim.step()).collect()
}

#[test]
fn another_protocol_or_version_is_refused() {
    let mut sim = Sim::new(LinkConditions::default(), config());
    sim.join(
        LinkConditions::default(),
        NetConfig {
            protocol: 1,
            ..config()
        },
    );
    sim.join(
        LinkConditions::default(),
        NetConfig {
            channels: vec![ChannelKind::ReliableOrdered],
            ..config()
        },
    );

    let events = client_events(&mut sim, 20);

    for client in [0, 1] {
        assert!(
            events.contains(&(
                Some(client),
                NetEvent::Disconnected {
                    peer: PeerId::SERVER,
                    reason: DisconnectReason::ProtocolMismatch
                }
            )),
            "{events:?}"
        );
    }
    assert_eq!(sim.server.peer_count(), 0);
}

#[test]
fn a_full_server_refuses_one_more() {
    let mut sim = Sim::new(
        LinkConditions::default(),
        NetConfig {
            max_clients: 2,
            ..config()
        },
    );
    for _ in 0..3 {
        sim.join(LinkConditions::default(), config());
    }

    let events = client_events(&mut sim, 20);

    let refused: Vec<_> = events
        .iter()
        .filter(|(_, e)| {
            *e == NetEvent::Disconnected {
                peer: PeerId::SERVER,
                reason: DisconnectReason::ServerFull,
            }
        })
        .collect();
    assert_eq!(refused.len(), 1, "{events:?}");
    assert_eq!(sim.server.peer_count(), 2);
}

#[test]
fn leaving_and_kicking_are_seen_by_both_ends_with_their_reasons() {
    let mut sim = Sim::new(LinkConditions::default(), config());
    sim.join(LinkConditions::default(), config());
    sim.join(LinkConditions::default(), config());
    sim.connect_all();

    sim.clients[0].disconnect(PeerId::SERVER);
    sim.server.kick(PeerId(2));
    let events = client_events(&mut sim, 5);

    for expected in [
        (
            Some(0),
            NetEvent::Disconnected {
                peer: PeerId::SERVER,
                reason: DisconnectReason::Left,
            },
        ),
        (
            None,
            NetEvent::Disconnected {
                peer: PeerId(1),
                reason: DisconnectReason::Closed,
            },
        ),
        (
            None,
            NetEvent::Disconnected {
                peer: PeerId(2),
                reason: DisconnectReason::Kicked,
            },
        ),
        (
            Some(1),
            NetEvent::Disconnected {
                peer: PeerId::SERVER,
                reason: DisconnectReason::Kicked,
            },
        ),
    ] {
        assert_eq!(
            events.iter().filter(|e| **e == expected).count(),
            1,
            "{expected:?} once in {events:?}"
        );
    }
    assert_eq!(sim.server.peer_count(), 0);
    assert!(sim.clients.iter().all(|c| c.peer_count() == 0));
}

#[test]
fn silence_times_a_connection_out_and_a_missing_server_never_answers() {
    let quick = NetConfig {
        timeout: Duration::from_millis(500),
        connect_timeout: Duration::from_millis(400),
        ..config()
    };
    let mut sim = Sim::new(LinkConditions::default(), quick.clone());
    sim.join(LinkConditions::default(), quick.clone());
    sim.connect_all();

    // The server goes quiet: its client times out, while it is stepped alone.
    let mut events = Vec::new();
    for _ in 0..50 {
        sim.now += TICK;
        let client = &mut sim.clients[0];
        client.receive(sim.now);
        events.extend(client.events().iter().copied());
        client.flush(sim.now);
    }
    assert!(events.contains(&NetEvent::Disconnected {
        peer: PeerId::SERVER,
        reason: DisconnectReason::Timeout
    }));

    // And the other way round: the server notices the silent client.
    let mut events = Vec::new();
    for _ in 0..50 {
        sim.now += TICK;
        sim.server.receive(sim.now);
        events.extend(sim.server.events().iter().copied());
        sim.server.flush(sim.now);
    }
    assert!(events.contains(&NetEvent::Disconnected {
        peer: PeerId(1),
        reason: DisconnectReason::Timeout
    }));

    // A client dialling an address nobody listens at gives up.
    let network = MemoryNetwork::new();
    let nobody = network.endpoint().address();
    let mut lonely = Endpoint::client(network.endpoint(), nobody, quick);
    let mut events = Vec::new();
    for tick in 1..60u32 {
        lonely.update(TICK * tick);
        events.extend(lonely.events().iter().copied());
    }
    assert_eq!(
        events,
        vec![NetEvent::Disconnected {
            peer: PeerId::SERVER,
            reason: DisconnectReason::ConnectTimeout
        }]
    );
}

#[test]
fn a_dropped_endpoint_says_goodbye() {
    let mut sim = Sim::new(LinkConditions::default(), config());
    sim.join(LinkConditions::default(), config());
    sim.connect_all();

    drop(sim.clients.pop());
    let events = client_events(&mut sim, 3);

    assert!(events.contains(&(
        None,
        NetEvent::Disconnected {
            peer: PeerId(1),
            reason: DisconnectReason::Closed
        }
    )));
}

/// Random bytes at the server from strangers, from a connected client's
/// address, and as the frames of that client's real packets: nothing
/// panics, the strangers get nowhere, and the tampered client is cut off
/// for a violation while an honest one carries on.
#[test]
fn hostile_bytes_never_panic_and_a_tampering_peer_is_cut_off() {
    let mut sim = Sim::new(LinkConditions::default(), config());
    sim.join(LinkConditions::default(), config());
    let tamper = Tamper {
        inner: sim.network.endpoint(),
        rng: 77,
        armed: false,
    };
    sim.clients
        .push(Endpoint::client(tamper, sim.server_addr, config()));
    sim.connect_all();
    let mut stranger = sim.network.endpoint();
    let mut rng = 12345u64;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };

    for _ in 0..2_000 {
        let len = (next() % 1201) as usize;
        let mut bytes: Vec<u8> = (0..len).map(|_| next() as u8).collect();
        if let Some(kind) = bytes.first_mut() {
            *kind = (*kind % 8).max(1);
        }
        stranger
            .send(sim.server_addr, &bytes)
            .expect("a datagram of at most the limit");
    }
    let mut events = Vec::new();
    for tick in 0..100 {
        // The tampering client's first message arms it; the ones after it
        // go as junk.
        let _ = sim.clients[1].send(PeerId::SERVER, ChannelId::RELIABLE_ORDERED, b"more");
        if tick % 2 == 0 {
            let _ = sim.clients[0].send(PeerId::SERVER, ChannelId::RELIABLE_ORDERED, b"hi");
        }
        events.extend(sim.step());
    }

    assert!(
        events.contains(&(
            None,
            NetEvent::Disconnected {
                peer: PeerId(2),
                reason: DisconnectReason::Violation
            }
        )),
        "{events:?}"
    );
    assert_eq!(sim.server.peers().collect::<Vec<_>>(), vec![PeerId(1)]);
    assert!(sim.clients[0].is_connected(PeerId::SERVER));
}

/// A transport that, once it has carried a payload with a frame in it,
/// replaces the frames of every later payload with random bytes — leaving
/// the header, and so the session, intact.
struct Tamper {
    inner: MemoryTransport,
    rng: u64,
    armed: bool,
}

impl Transport for Tamper {
    fn send(&mut self, to: Address, payload: &[u8]) -> Result<(), TransportError> {
        const HEADER: usize = 17;
        let mut bytes = payload.to_vec();
        if bytes.first() == Some(&6) && bytes.len() > HEADER {
            if self.armed {
                for byte in &mut bytes[HEADER..] {
                    self.rng ^= self.rng << 13;
                    self.rng ^= self.rng >> 7;
                    self.rng ^= self.rng << 17;
                    *byte = self.rng as u8;
                }
            }
            self.armed = true;
        }
        self.inner.send(to, &bytes)
    }

    fn recv(&mut self) -> Result<Option<Datagram>, TransportError> {
        self.inner.recv()
    }

    fn reaches(&self, to: Address) -> bool {
        self.inner.reaches(to)
    }

    fn local_addr(&self) -> Option<Address> {
        self.inner.local_addr()
    }
}
