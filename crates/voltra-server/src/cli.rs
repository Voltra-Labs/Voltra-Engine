//! The dedicated server's command line and run, for `voltra-server` and for
//! a game's own server binary.
//!
//! A game with server logic builds its binary on this, as it builds its
//! player on `App::run`: `voltra_server::cli::main("my-server", |args| app)`
//! — the same flags (`--udp`, `--ws`, `--seconds`, `--port-file`, …), its
//! own game in the app, and a chance to set defaults (its first scene, its
//! protocol number) on the parsed arguments before they are used.

use std::process::ExitCode;
use std::time::Instant;

use voltra_assets::AssetPath;
use voltra_core::net::Address;
use voltra_core::App;

use crate::args::{self, Args, Parsed};
use crate::Server;

/// Parses the command line, builds the app with `make` (which may adjust
/// the arguments first), hosts it and runs it until killed or `--seconds`
/// pass. `name` is what the usage calls the program.
pub fn main(name: &str, make: impl FnOnce(&mut Args) -> App) -> ExitCode {
    voltra_core::logging::init(false);
    let usage = args::USAGE.replace("voltra-server", name);
    let mut args = match args::parse(std::env::args().skip(1)) {
        Ok(Parsed::Run(args)) => args,
        Ok(Parsed::Help) => {
            println!("{usage}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            log::error!("{e}\n\n{usage}");
            return ExitCode::FAILURE;
        }
    };
    let app = make(&mut args);
    let flags = args.lobby.clone();
    flags.apply(&mut args.options);
    match run(app, *args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn run(mut app: App, args: Args) -> Result<(), String> {
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
    if let Some(at) = server.discovery_addr() {
        log::info!("answering LAN probes on udp://{at}");
    }
    if let Some(file) = &args.port_file {
        let mut lines: String = addresses
            .iter()
            .filter_map(|address| match address {
                Address::Udp(at) => Some(format!("udp {}\n", at.port())),
                Address::WebSocket(at) => Some(format!("ws {}\n", at.port())),
                _ => None,
            })
            .collect();
        if let Some(at) = server.discovery_addr() {
            lines.push_str(&format!("discovery {}\n", at.port()));
        }
        std::fs::write(file, lines)
            .map_err(|e| format!("could not write {}: {e}", file.display()))?;
    }

    let start = Instant::now();
    server.run(|_| args.seconds.is_none_or(|limit| start.elapsed() < limit));
    server.shut_down();
    log::info!("stopped after {} ticks", server.ticks());
    Ok(())
}
