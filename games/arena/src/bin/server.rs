//! The arena's dedicated server: the match, headless at a fixed tick,
//! hosted over UDP and WebSockets — `voltra-server`'s command line with
//! the arena's rules in it.
//!
//!   cargo run -p arena --bin arena-server -- --asset-root games/arena/assets

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    use arena::{
        lobby_config, replication_config, Arena, Pilot, Session, FIRST_SCENE, GAME_ID, PROTOCOL,
    };
    use voltra_core::{App, WindowConfig};

    voltra_server::cli::main("arena-server", |args| {
        args.scene.get_or_insert_with(|| FIRST_SCENE.to_owned());
        args.options.host.config.protocol = PROTOCOL;
        // A lobby before the match: the first bot or player in starts it
        // (`--min-players`, `--max-players` change its rules).
        args.options.lobby.get_or_insert_with(lobby_config);
        // Found on the LAN as an arena: its defaults, which `--name`,
        // `--game` and `--discovery off` override.
        if let Some(advertise) = args.options.host.discovery.as_mut() {
            advertise.game = GAME_ID.to_owned();
            advertise.name = "Arena server".to_owned();
        }
        App::new(WindowConfig {
            title: "Arena server".into(),
            ..WindowConfig::default()
        })
        .with_replication(replication_config())
        // The server hosts before the game's first turn, so the game's own
        // session is never started; a server has no player of its own.
        .with_game(Arena::new(Session::Solo, Pilot::Bot))
    })
}

/// A browser cannot host: the web build has no server.
#[cfg(target_arch = "wasm32")]
fn main() {}
