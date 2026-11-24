//! A native server that sends every message back to whoever sent it, on
//! the channel it came on: what the web probe (`web_probe.rs`) connects to
//! in `crates/voltra-player/web/test/net.mjs`.
//!
//!   cargo run -p voltra-net --example echo_server -- --port-file PATH [--seconds N]
//!
//! Listens for WebSockets on a free loopback port and writes the port to
//! `PATH`, so the test needs no fixed port; exits after `N` seconds
//! (default 60).

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    use std::time::{Duration, Instant};
    use voltra_net::{Endpoint, NetConfig, NetEvent, WebSocketServer};

    env_logger::init();
    let args: Vec<String> = std::env::args().collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
    };
    let Some(port_file) = value("--port-file") else {
        eprintln!("usage: echo_server --port-file PATH [--seconds N]");
        return std::process::ExitCode::FAILURE;
    };
    let seconds = value("--seconds")
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);

    let listener = match WebSocketServer::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(e) => {
            log::error!("could not listen: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let port = listener.local_socket().port();
    let mut server = Endpoint::server(listener, NetConfig::default());
    if let Err(e) = std::fs::write(port_file, port.to_string()) {
        log::error!("could not write {port_file}: {e}");
        return std::process::ExitCode::FAILURE;
    }

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(seconds) {
        server.receive(start.elapsed());
        for event in server.events() {
            match event {
                NetEvent::Connected(peer) => log::info!("{peer} joined"),
                NetEvent::Disconnected { peer, reason } => log::info!("{peer} left: {reason}"),
            }
        }
        for message in server.take_messages() {
            if let Err(e) = server.send(message.from, message.channel, &message.bytes) {
                log::error!("could not answer {}: {e}", message.from);
            }
        }
        server.flush(start.elapsed());
        std::thread::sleep(Duration::from_millis(5));
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(target_arch = "wasm32")]
fn main() {}
