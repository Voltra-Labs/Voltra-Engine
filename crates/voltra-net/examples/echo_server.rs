//! A native WebSocket server that sends every datagram back: what the web
//! probe (`web_probe.rs`) connects to in `crates/voltra-player/web/test/net.mjs`.
//!
//!   cargo run -p voltra-net --example echo_server -- --port-file PATH [--seconds N]
//!
//! Listens on a free loopback port and writes the port to `PATH`, so the
//! test needs no fixed port; exits after `N` seconds (default 60).

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    use std::time::{Duration, Instant};
    use voltra_net::{Transport, WebSocketServer};

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

    let mut server = match WebSocketServer::bind("127.0.0.1:0") {
        Ok(server) => server,
        Err(e) => {
            log::error!("could not listen: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    if let Err(e) = std::fs::write(port_file, server.local_socket().port().to_string()) {
        log::error!("could not write {port_file}: {e}");
        return std::process::ExitCode::FAILURE;
    }

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(seconds) {
        if let Err(e) = server.update(start.elapsed()) {
            log::error!("the server failed: {e}");
            return std::process::ExitCode::FAILURE;
        }
        while let Ok(Some(datagram)) = server.recv() {
            if let Err(e) = server.send(datagram.from, &datagram.payload) {
                log::error!("could not answer {}: {e}", datagram.from);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(target_arch = "wasm32")]
fn main() {}
