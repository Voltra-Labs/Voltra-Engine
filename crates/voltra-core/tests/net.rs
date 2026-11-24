//! `tick.net`: two headless games over an in-memory network, the server's
//! side of the link a bad one — one hosts, one joins, the client says hello, the server answers
//! through the tick, and both see who came and went.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use voltra_core::net::{
    ChannelId, Conditioned, DisconnectReason, LinkConditions, MemoryNetwork, NetConfig, NetEvent,
    PeerId,
};
use voltra_core::{App, Headless};

const FRAME: f32 = 1.0 / 60.0;

#[derive(Default)]
struct Seen {
    events: Vec<NetEvent>,
    messages: Vec<(PeerId, String)>,
}

fn link() -> LinkConditions {
    LinkConditions {
        latency: Duration::from_millis(20),
        jitter: Duration::from_millis(20),
        loss: 0.1,
        duplicate: 0.0,
        bandwidth: 0,
    }
}

#[test]
fn a_hosted_game_and_a_joined_one_talk_through_their_ticks() {
    let network = MemoryNetwork::new();
    let server_transport = Conditioned::new(network.endpoint(), link(), 1);
    let server_addr = server_transport.inner().address();
    // The client's own sends go straight out: its goodbye is not held in a
    // simulated delay the session's end would cut short.
    let client_transport = Conditioned::new(network.endpoint(), LinkConditions::default(), 2);

    let on_server = Rc::new(RefCell::new(Seen::default()));
    let log = on_server.clone();
    let mut server = Headless::new(App::default().with_update(move |tick| {
        let mut seen = log.borrow_mut();
        seen.events.extend(tick.net.events().iter().copied());
        for message in tick.net.take_received() {
            let text: String = message.decode().expect("a string");
            tick.net
                .send_message(
                    message.from,
                    ChannelId::RELIABLE_ORDERED,
                    &format!("hi, {text}"),
                )
                .expect("queues");
            seen.messages.push((message.from, text));
        }
    }));
    server
        .net_mut()
        .host_on(server_transport, NetConfig::default());

    let on_client = Rc::new(RefCell::new(Seen::default()));
    let log = on_client.clone();
    let mut client = Headless::new(App::default().with_update(move |tick| {
        let mut seen = log.borrow_mut();
        // Copied out: the events borrow the session the reply is sent on.
        for event in tick.net.events().to_vec() {
            if event == NetEvent::Connected(PeerId::SERVER) {
                tick.net
                    .send_message(PeerId::SERVER, ChannelId::RELIABLE_ORDERED, "ada")
                    .expect("queues");
            }
        }
        seen.events.extend(tick.net.events().iter().copied());
        for message in tick.net.received() {
            seen.messages
                .push((message.from, message.decode().expect("a string")));
        }
    }));
    client
        .net_mut()
        .join_on(client_transport, server_addr, NetConfig::default());

    for _ in 0..240 {
        server.step(1, FRAME);
        client.step(1, FRAME);
    }
    assert_eq!(client.net().local_id(), Some(PeerId(1)));
    assert!(server.net().is_server() && !client.net().is_server());
    client.net_mut().stop();
    for _ in 0..30 {
        server.step(1, FRAME);
        client.step(1, FRAME);
    }

    let server_seen = on_server.borrow();
    assert_eq!(
        server_seen.events,
        vec![
            NetEvent::Connected(PeerId(1)),
            NetEvent::Disconnected {
                peer: PeerId(1),
                reason: DisconnectReason::Closed
            }
        ]
    );
    assert_eq!(server_seen.messages, vec![(PeerId(1), "ada".to_owned())]);
    let client_seen = on_client.borrow();
    assert_eq!(
        client_seen.events,
        vec![
            NetEvent::Connected(PeerId::SERVER),
            NetEvent::Disconnected {
                peer: PeerId::SERVER,
                reason: DisconnectReason::Left
            }
        ],
        "the client hears its own goodbye before the session goes"
    );
    assert_eq!(
        client_seen.messages,
        vec![(PeerId::SERVER, "hi, ada".to_owned())]
    );
    assert!(!client.net().is_active(), "and then it is gone");
}

/// A game that hosts and plays — a listen server — on real sockets: its
/// own player joins first, as `PeerId::SERVER`, then a client joining by
/// address; stopping the session ends both.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_listen_server_s_own_player_joins_like_any_other() {
    use voltra_core::net::{Address, HostOptions};

    let joins = Rc::new(RefCell::new(Vec::new()));
    let log = joins.clone();
    let mut host = Headless::new(App::default().with_update(move |tick| {
        log.borrow_mut().extend(tick.net.events().iter().copied());
    }));
    let options = HostOptions {
        bind: [127, 0, 0, 1].into(),
        udp_port: Some(0),
        websocket_port: Some(0),
        listen: true,
        ..HostOptions::default()
    };
    host.net_mut().host(&options).expect("the ports open");
    let addrs = host.net().endpoint().expect("hosting").local_addrs();
    let Some(Address::Udp(udp)) = addrs.iter().find(|a| matches!(a, Address::Udp(_))).copied()
    else {
        panic!("a UDP address in {addrs:?}");
    };
    assert!(addrs.iter().any(|a| matches!(a, Address::WebSocket(_))));

    let mut client = Headless::new(App::default());
    client
        .net_mut()
        .join(&format!("udp://{udp}"), NetConfig::default())
        .expect("a server address");
    for _ in 0..200 {
        host.step(1, FRAME);
        client.step(1, FRAME);
        std::thread::sleep(Duration::from_millis(1));
        if host.net().players().len() == 2 && client.net().local_id().is_some() {
            break;
        }
    }
    assert!(host.net().is_listen_server());
    assert_eq!(host.net().players(), vec![PeerId::SERVER, PeerId(1)]);
    assert_eq!(client.net().players(), vec![PeerId(1)]);

    host.net_mut().stop();
    host.step(1, FRAME);
    assert_eq!(
        *joins.borrow(),
        vec![
            NetEvent::Connected(PeerId::SERVER),
            NetEvent::Connected(PeerId(1)),
            NetEvent::Disconnected {
                peer: PeerId::SERVER,
                reason: DisconnectReason::Left
            },
            NetEvent::Disconnected {
                peer: PeerId(1),
                reason: DisconnectReason::Left
            },
        ]
    );
    let mut told = Vec::new();
    for _ in 0..50 {
        client.step(1, FRAME);
        told.extend(client.net().events().iter().copied());
        std::thread::sleep(Duration::from_millis(1));
        if !told.is_empty() {
            break;
        }
    }
    assert_eq!(
        told,
        vec![NetEvent::Disconnected {
            peer: PeerId::SERVER,
            reason: DisconnectReason::Shutdown
        }]
    );
}
