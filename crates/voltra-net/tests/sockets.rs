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
