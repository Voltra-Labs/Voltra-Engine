//! `voltra-server`: a scene run headless at a fixed tick, hosted for
//! clients. See [`voltra_server`] for the library a game's own server
//! binary is built on.

mod args;

use std::process::ExitCode;
use std::time::Instant;

use voltra_assets::AssetPath;
use voltra_core::net::Address;
use voltra_core::{App, WindowConfig};
use voltra_server::Server;

use args::{Args, Parsed};

fn main() -> ExitCode {
    voltra_core::logging::init(false);
    let args = match args::parse(std::env::args().skip(1)) {
        Ok(Parsed::Run(args)) => args,
        Ok(Parsed::Help) => {
            println!("{}", args::USAGE);
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            log::error!("{e}\n\n{}", args::USAGE);
            return ExitCode::FAILURE;
        }
    };
    match run(*args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<(), String> {
    let mut app = App::new(WindowConfig {
        title: "voltra-server".into(),
        ..WindowConfig::default()
    });
    if let Some(root) = &args.asset_root {
        app = app.with_asset_root(root);
    }
    let mut server = match &args.scene {
        Some(scene) => {
            let scene = AssetPath::new(scene).map_err(|e| format!("{scene:?}: {e}"))?;
            Server::open(app, &scene, &args.options)
        }
        None => Server::new(app, &args.options),
    }
    .map_err(|e| e.to_string())?;

    let addresses = server.addresses();
    for address in &addresses {
        log::info!("listening on {address}");
    }
    if let Some(file) = &args.port_file {
        let lines: String = addresses
            .iter()
            .filter_map(|address| match address {
                Address::Udp(at) => Some(format!("udp {}\n", at.port())),
                Address::WebSocket(at) => Some(format!("ws {}\n", at.port())),
                _ => None,
            })
            .collect();
        std::fs::write(file, lines)
            .map_err(|e| format!("could not write {}: {e}", file.display()))?;
    }

    let start = Instant::now();
    server.run(|_| args.seconds.is_none_or(|limit| start.elapsed() < limit));
    server.shut_down();
    log::info!("stopped after {} ticks", server.ticks());
    Ok(())
}
