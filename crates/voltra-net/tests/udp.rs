//! A native client and server exchange datagrams over UDP on the loopback.

#![cfg(not(target_arch = "wasm32"))]

use std::net::UdpSocket;
use std::time::{Duration, Instant};

use voltra_net::{Address, Datagram, Transport, UdpTransport, MAX_DATAGRAM};

/// Polls `transport` until a datagram arrives or two seconds pass.
fn wait_for(transport: &mut impl Transport) -> Option<Datagram> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        transport.update(start.elapsed()).expect("updates");
        if let Some(datagram) = transport.recv().expect("reads") {
            return Some(datagram);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    None
}

#[test]
fn a_client_and_a_server_exchange_datagrams() {
    let mut server = UdpTransport::bind("127.0.0.1:0").expect("the server binds");
    let mut client = UdpTransport::bind("127.0.0.1:0").expect("the client binds");
    let server_addr = Address::Udp(server.local_socket());

    client.send(server_addr, b"ping").expect("sends");
    let request = wait_for(&mut server).expect("the server hears the client");
    assert_eq!(request.payload, b"ping");
    assert_eq!(request.from, client.local_addr().expect("bound"));

    server.send(request.from, b"pong").expect("answers");
    let reply = wait_for(&mut client).expect("the client hears the answer");
    assert_eq!(reply.payload, b"pong");
    assert_eq!(reply.from, server_addr);
}

#[test]
fn the_largest_datagram_arrives_whole_and_a_longer_one_is_dropped() {
    let mut server = UdpTransport::bind("127.0.0.1:0").expect("the server binds");
    let raw = UdpSocket::bind("127.0.0.1:0").expect("a raw socket binds");
    let to = server.local_socket();

    // Off the wire, not through our send — which would refuse it.
    raw.send_to(&[1; MAX_DATAGRAM + 1], to).expect("sends");
    raw.send_to(&[2; MAX_DATAGRAM], to).expect("sends");

    let arrived = wait_for(&mut server).expect("the second arrives");
    assert_eq!(arrived.payload, vec![2; MAX_DATAGRAM]);
    assert!(server.recv().expect("reads").is_none(), "the first did not");
}
