//! Connections over real sockets: one server on UDP and WebSockets at once,
//! a UDP client and a WebSocket client in, a message each way.

#![cfg(not(target_arch = "wasm32"))]

use std::time::{Duration, Instant};

use voltra_net::{
    Address, ChannelId, Endpoint, MultiTransport, NetConfig, NetEvent, PeerId, UdpTransport,
    WebSocketClient, WebSocketServer,
};

#[test]
fn a_udp_client_and_a_websocket_client_join_one_server() {
    let udp = UdpTransport::bind("127.0.0.1:0").expect("binds");
    let ws = WebSocketServer::bind("127.0.0.1:0").expect("listens");
    let udp_addr = Address::Udp(udp.local_socket());
    let ws_socket = ws.local_socket();
    let mut server = Endpoint::server(
        MultiTransport::new().with(udp).with(ws),
        NetConfig::default(),
    );

    let mut native = Endpoint::client(
        UdpTransport::bind("127.0.0.1:0").expect("binds"),
        udp_addr,
        NetConfig::default(),
    );
    let web = WebSocketClient::connect(ws_socket);
    let web_server = web.server();
    let mut browser_like = Endpoint::client(web, web_server, NetConfig::default());

    let start = Instant::now();
    let mut joined = Vec::new();
    let mut heard = Vec::new();
    let mut answered = [false; 2];
    while start.elapsed() < Duration::from_secs(5) && !(heard.len() == 2 && answered == [true; 2]) {
        let now = start.elapsed();
        server.receive(now);
        for event in server.events().to_vec() {
            if let NetEvent::Connected(peer) = event {
                joined.push(peer);
                server
                    .send(peer, ChannelId::RELIABLE_ORDERED, b"welcome")
                    .expect("queues");
            }
        }
        heard.extend(server.messages().iter().map(|m| m.bytes.clone()));
        server.flush(now);
        for (i, client) in [&mut native, &mut browser_like].into_iter().enumerate() {
            client.receive(now);
            if client
                .events()
                .contains(&NetEvent::Connected(PeerId::SERVER))
            {
                let hello = if i == 0 { &b"udp"[..] } else { &b"ws"[..] };
                client
                    .send(PeerId::SERVER, ChannelId::RELIABLE_ORDERED, hello)
                    .expect("queues");
            }
            if client.messages().iter().any(|m| m.bytes == b"welcome") {
                answered[i] = true;
            }
            client.flush(now);
        }
        std::thread::sleep(Duration::from_millis(2));
    }

    joined.sort();
    assert_eq!(joined, vec![PeerId(1), PeerId(2)]);
    heard.sort();
    assert_eq!(heard, vec![b"udp".to_vec(), b"ws".to_vec()]);
    assert_eq!(answered, [true, true]);
}

/// A stream the server closes while the client's datagrams are still
/// unread on it: the server's last datagram must reach the client before
/// the close. Dropping the TCP stream at once would answer the unread
/// bytes with a reset, and Windows then discards what the client had not
/// read yet — a kick's reason among it.
#[test]
fn the_last_datagram_before_a_close_reaches_the_peer() {
    use voltra_net::Transport;

    let mut server = WebSocketServer::bind("127.0.0.1:0").expect("listens");
    let mut client = WebSocketClient::connect(server.local_socket());
    let to_server = client.server();
    let start = Instant::now();

    // Open, and learn the client's address from its first datagram.
    let mut peer = None;
    while peer.is_none() {
        assert!(start.elapsed() < Duration::from_secs(5), "never opened");
        let now = start.elapsed();
        client.update(now).expect("client");
        let _ = client.send(to_server, b"hello");
        server.update(now).expect("server");
        while let Some(datagram) = server.recv().expect("recv") {
            peer = Some(datagram.from);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let peer = peer.expect("heard");

    // Bytes the server will never read before it closes.
    let filler = vec![7u8; 1000];
    for _ in 0..200 {
        let _ = client.send(to_server, &filler);
    }
    client.update(start.elapsed()).expect("flushes");
    std::thread::sleep(Duration::from_millis(50));

    server.send(peer, b"bye").expect("queues");
    server.close(peer);

    let mut heard_bye = false;
    let deadline = Instant::now() + Duration::from_secs(3);
    while !heard_bye && Instant::now() < deadline {
        let now = start.elapsed();
        server.update(now).expect("server");
        if client.update(now).is_err() {
            // A closed transport still hands over what arrived first.
        }
        while let Ok(Some(datagram)) = client.recv() {
            heard_bye |= datagram.payload == b"bye";
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(heard_bye, "the datagram sent before the close arrived");
}
