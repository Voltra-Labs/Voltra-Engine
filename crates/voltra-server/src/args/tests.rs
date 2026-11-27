//! The command line as people type it.

use super::*;
use voltra_core::net::LobbyConfig;

fn run(args: &[&str]) -> Result<Parsed, ArgError> {
    parse(args.iter().map(|a| a.to_string()))
}

#[test]
fn the_defaults_host_both_transports_at_sixty_ticks() {
    let Ok(Parsed::Run(args)) = run(&[]) else {
        panic!("runs");
    };
    assert_eq!(args.options.tick_rate, 60);
    assert_eq!(args.options.host.udp_port, Some(7777));
    assert_eq!(args.options.host.websocket_port, Some(7778));
    assert_eq!(
        args.options.host.discovery.as_ref().map(|d| d.port),
        Some(7779),
        "found on the LAN by default"
    );
    assert_eq!(args.options.lobby, None, "no lobby unless asked");
    assert_eq!(args.scene, None);
}

#[test]
fn lobby_flags_lay_over_the_lobby_a_game_set() {
    let Ok(Parsed::Run(mut args)) = run(&["--min-players", "1", "--discovery", "off"]) else {
        panic!("runs");
    };
    args.discovery.apply(&mut args.options);
    assert_eq!(args.options.host.discovery, None);
    // The game's server says its lobby first; the flags win.
    args.options.lobby = Some(LobbyConfig {
        max_players: 4,
        ..LobbyConfig::default()
    });
    args.lobby.apply(&mut args.options);
    let lobby = args.options.lobby.expect("a lobby");
    assert_eq!((lobby.min_players, lobby.max_players), (1, 4));

    let Ok(Parsed::Run(mut plain)) = run(&["--lobby"]) else {
        panic!("runs");
    };
    plain.lobby.apply(&mut plain.options);
    assert_eq!(plain.options.lobby, Some(LobbyConfig::default()));
}

#[test]
fn every_flag_is_read() {
    let Ok(Parsed::Run(mut args)) = run(&[
        "scenes/arena.ron",
        "--asset-root",
        "game/assets",
        "--bind",
        "127.0.0.1",
        "--udp",
        "0",
        "--ws",
        "off",
        "--tick",
        "30",
        "--max-clients",
        "8",
        "--protocol",
        "42",
        "--seconds",
        "5",
        "--port-file",
        "ports.txt",
        "--discovery",
        "0",
        "--name",
        "Ada's server",
        "--game",
        "arena",
        "--max-players",
        "6",
    ]) else {
        panic!("runs");
    };
    assert_eq!(args.scene.as_deref(), Some("scenes/arena.ron"));
    assert_eq!(args.asset_root, Some(PathBuf::from("game/assets")));
    assert_eq!(args.options.host.bind, IpAddr::from([127, 0, 0, 1]));
    assert_eq!(args.options.host.udp_port, Some(0));
    assert_eq!(args.options.host.websocket_port, None);
    assert_eq!(args.options.tick_rate, 30);
    assert_eq!(args.options.host.config.max_clients, 8);
    assert_eq!(args.options.host.config.protocol, 42);
    assert_eq!(args.seconds, Some(Duration::from_secs(5)));
    assert_eq!(args.port_file, Some(PathBuf::from("ports.txt")));
    args.apply_flags();
    let advertise = args.options.host.discovery.as_ref().expect("on");
    assert_eq!(advertise.port, 0);
    assert_eq!(advertise.name, "Ada's server");
    assert_eq!(advertise.game, "arena");
    assert_eq!(args.lobby.max_players, Some(6));
    assert!(args.lobby.on);
}

#[test]
fn mistakes_are_refused() {
    assert_eq!(run(&["--tick"]), Err(ArgError::MissingValue("--tick")));
    assert_eq!(
        run(&["--tick", "0"]),
        Err(ArgError::BadValue("--tick", "0".into()))
    );
    assert_eq!(
        run(&["--udp", "seventy"]),
        Err(ArgError::BadValue("--udp", "seventy".into()))
    );
    assert_eq!(
        run(&["--loud"]),
        Err(ArgError::UnknownFlag("--loud".into()))
    );
    assert_eq!(
        run(&["a.ron", "b.ron"]),
        Err(ArgError::ExtraScene("b.ron".into()))
    );
    assert_eq!(run(&["--help"]), Ok(Parsed::Help));
}

#[test]
fn discovery_flags_are_read_in_any_order_and_win_over_the_game() {
    use voltra_core::net::{Advertise, DiscoveryScope};

    // The name before the port, after an off taken back: all of it kept.
    let Ok(Parsed::Run(mut args)) = run(&[
        "--discovery",
        "off",
        "--name",
        "Ada's server",
        "--discovery",
        "7790",
        "--game",
        "chess",
    ]) else {
        panic!("runs");
    };
    // The game's server sets its own defaults first, as arena's does.
    if let Some(advertise) = args.options.host.discovery.as_mut() {
        advertise.name = "Arena server".into();
        advertise.game = "arena".into();
    }
    args.apply_flags();
    let advertise = args.options.host.discovery.as_ref().expect("on");
    assert_eq!(
        (
            advertise.port,
            advertise.name.as_str(),
            advertise.game.as_str()
        ),
        (7790, "Ada's server", "chess"),
        "the command line wins"
    );
    assert_eq!(
        advertise.scope,
        DiscoveryScope::Local,
        "the LAN only, by default"
    );

    // An explicit name that happens to be the engine's default still wins.
    let Ok(Parsed::Run(mut args)) = run(&["--name", "Voltra server"]) else {
        panic!("runs");
    };
    if let Some(advertise) = args.options.host.discovery.as_mut() {
        advertise.name = "Arena server".into();
    }
    args.apply_flags();
    assert_eq!(
        args.options.host.discovery.map(|d| d.name),
        Some(Advertise::default().name)
    );

    // Public: any source answered. Off last: off, whatever else.
    let Ok(Parsed::Run(mut args)) = run(&["--discovery-public"]) else {
        panic!("runs");
    };
    args.apply_flags();
    assert_eq!(
        args.options.host.discovery.map(|d| d.scope),
        Some(DiscoveryScope::Anyone)
    );
    let Ok(Parsed::Run(mut args)) = run(&["--name", "x", "--discovery", "off"]) else {
        panic!("runs");
    };
    args.apply_flags();
    assert_eq!(args.options.host.discovery, None);

    // A game that turned discovery off is turned back on by a flag asking
    // for it.
    let Ok(Parsed::Run(mut args)) = run(&["--game", "chess"]) else {
        panic!("runs");
    };
    args.options.host.discovery = None;
    args.apply_flags();
    assert_eq!(
        args.options.host.discovery.map(|d| d.game),
        Some("chess".to_owned())
    );
}
