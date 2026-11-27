//! A native client and server exchange datagrams over a WebSocket on the
//! loopback, and the listener holds against peers that break its limits.

#![cfg(not(target_arch = "wasm32"))]

use std::io::Write as _;
use std::net::TcpStream;
use std::time::{Duration, Instant};

use voltra_net::{
    Address, Datagram, Transport, WebSocketClient, WebSocketLimits, WebSocketServer, MAX_DATAGRAM,
};

/// Updates both ends until `done` holds or three seconds pass.
fn pump_until(
    server: &mut WebSocketServer,
    client: &mut WebSocketClient,
    mut done: impl FnMut(&mut WebSocketServer, &mut WebSocketClient) -> bool,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        server.update(start.elapsed()).expect("updates");
        client.update(start.elapsed()).expect("updates");
        if done(server, client) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    false
}

fn received(transport: &mut impl Transport) -> Option<Datagram> {
    transport.recv().expect("reads")
}

#[test]
fn a_client_and_a_server_exchange_datagrams() {
    let mut server = WebSocketServer::bind("127.0.0.1:0").expect("listens");
    let mut client = WebSocketClient::connect(server.local_socket());

    assert!(
        pump_until(&mut server, &mut client, |_, c| c.is_open()),
        "the websocket opens"
    );
    client.send(client.server(), b"ping").expect("sends");
    let mut request = None;
    assert!(pump_until(&mut server, &mut client, |s, _| {
        request = received(s);
        request.is_some()
    }));
    let request = request.expect("arrived");
    assert_eq!(request.payload, b"ping");
    assert!(matches!(request.from, Address::WebSocket(_)));

    server.send(request.from, b"pong").expect("answers");
    let mut reply = None;
    assert!(pump_until(&mut server, &mut client, |_, c| {
        reply = received(c);
        reply.is_some()
    }));
    let reply = reply.expect("arrived");
    assert_eq!(reply.payload, b"pong");
    assert_eq!(reply.from, client.server());
}

#[test]
fn a_client_that_goes_away_is_reported_closed() {
    let mut server = WebSocketServer::bind("127.0.0.1:0").expect("listens");
    let mut client = WebSocketClient::connect(server.local_socket());
    assert!(pump_until(&mut server, &mut client, |s, c| s
        .stream_count()
        == 1
        && c.is_open()));
    let peer = client.local_addr().expect("open");

    drop(client);

    let mut other = WebSocketClient::connect(server.local_socket());
    let mut closed = None;
    assert!(pump_until(&mut server, &mut other, |s, _| {
        closed = closed.or(s.take_closed());
        closed.is_some()
    }));
    assert_eq!(closed, Some(peer));
    assert!(matches!(
        server.send(peer, b"anyone?"),
        Err(voltra_net::TransportError::Unreachable(_))
    ));
}

#[test]
fn a_message_longer_than_a_datagram_ends_its_stream() {
    let mut server = WebSocketServer::bind("127.0.0.1:0").expect("listens");
    let addr = server.local_socket();
    // A hand-made client, so the oversized message is not refused at its end.
    let opened = std::thread::spawn(move || {
        let stream = TcpStream::connect(addr).expect("connects");
        tungstenite_free_handshake(stream)
    });
    let start = Instant::now();
    while !opened.is_finished() && start.elapsed() < Duration::from_secs(3) {
        server.update(start.elapsed()).expect("updates");
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut stream = opened.join().expect("the handshake thread ran");
    while server.stream_count() == 0 && start.elapsed() < Duration::from_secs(3) {
        server.update(start.elapsed()).expect("updates");
    }
    assert_eq!(server.stream_count(), 1);

    // One masked binary frame with a payload two bytes over the limit.
    let len = MAX_DATAGRAM + 2;
    let mut frame = vec![0x82, 0x80 | 126, (len >> 8) as u8, len as u8, 0, 0, 0, 0];
    frame.extend(std::iter::repeat_n(7u8, len));
    stream.write_all(&frame).expect("writes");

    let mut closed = None;
    while closed.is_none() && start.elapsed() < Duration::from_secs(3) {
        server.update(start.elapsed()).expect("updates");
        closed = server.take_closed();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(closed.is_some(), "the stream was ended");
    assert!(
        server.recv().expect("reads").is_none(),
        "and nothing was taken"
    );
}

#[test]
fn a_connection_that_never_shakes_hands_is_dropped_after_the_timeout() {
    // One handshake at a time: the real client gets in only if the silent
    // one's slot is freed when its time is up.
    let limits = WebSocketLimits {
        handshake_timeout: Duration::from_millis(50),
        max_handshakes: 1,
        ..Default::default()
    };
    let mut server = WebSocketServer::bind_with("127.0.0.1:0", limits).expect("listens");
    let _silent = TcpStream::connect(server.local_socket()).expect("connects");

    // The silent one is taken in on the caller's clock at zero (its
    // connect has returned, so it waits to be accepted), then the clock
    // passes the timeout.
    for _ in 0..5 {
        server.update(Duration::ZERO).expect("updates");
        std::thread::sleep(Duration::from_millis(2));
    }
    let after = Duration::from_millis(100);
    server.update(after).expect("updates");

    // The clock stands still from here: however slowly the machine runs,
    // the real client's own handshake never times out.
    let mut client = WebSocketClient::connect(server.local_socket());
    let start = Instant::now();
    while !client.is_open() && start.elapsed() < Duration::from_secs(10) {
        server.update(after).expect("updates");
        client.update(after).expect("updates");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        client.is_open(),
        "the slot was freed, and a real client got in"
    );
    assert_eq!(server.stream_count(), 1, "the silent one never did");
}

/// The client half of RFC 6455's opening handshake, by hand: a fixed key,
/// the response read up to its blank line.
fn tungstenite_free_handshake(mut stream: TcpStream) -> TcpStream {
    use std::io::Read as _;
    let request = "GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\n\
                   Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                   Sec-WebSocket-Version: 13\r\n\r\n";
    stream.write_all(request.as_bytes()).expect("writes");
    let mut response = Vec::new();
    let mut byte = [0u8; 1];
    while !response.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).expect("reads the response");
        response.push(byte[0]);
    }
    assert!(response.starts_with(b"HTTP/1.1 101"), "switched protocols");
    stream
}
