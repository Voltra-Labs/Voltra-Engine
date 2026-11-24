//! The arena's dedicated server: the match, headless at a fixed tick,
//! hosted over UDP and WebSockets — `voltra-server`'s command line with
//! the arena's rules in it.
//!
//!   cargo run -p arena --bin arena-server -- --asset-root games/arena/assets

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    use arena::{Arena, Pilot, Session, FIRST_SCENE, PROTOCOL};
    use voltra_core::replication::ReplicationConfig;
    use voltra_core::{App, WindowConfig};

    voltra_server::cli::main("arena-server", |args| {
        args.scene.get_or_insert_with(|| FIRST_SCENE.to_owned());
        args.options.host.config.protocol = PROTOCOL;
        App::new(WindowConfig {
            title: "Arena server".into(),
            ..WindowConfig::default()
        })
        .with_replication(ReplicationConfig::default())
        // The server hosts before the game's first turn, so the game's own
        // session is never started; a server has no player of its own.
        .with_game(Arena::new(Session::Solo, Pilot::Bot))
    })
}

/// A browser cannot host: the web build has no server.
#[cfg(target_arch = "wasm32")]
fn main() {}
