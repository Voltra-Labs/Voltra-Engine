//! The server's command line, which a game's own server binary shares
//! through [`crate::cli`].

use std::fmt;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

use crate::{ServerOptions, DEFAULT_TICK_RATE};
use voltra_core::net::{
    Advertise, HostOptions, LobbyConfig, NetConfig, DEFAULT_DISCOVERY_PORT, DEFAULT_UDP_PORT,
    DEFAULT_WEBSOCKET_PORT,
};

/// What `--help` prints.
pub const USAGE: &str = "\
Usage: voltra-server [OPTIONS] [SCENE]

Runs SCENE (an asset path, e.g. scenes/sandbox.ron) headless at a fixed
tick and hosts it for clients over UDP and WebSockets.

Options:
      --asset-root DIR   Directory every asset path resolves against
      --bind IP          Interface to listen on (default 0.0.0.0)
      --udp PORT|off     UDP port for native clients (default 7777)
      --ws PORT|off      WebSocket port for browser clients (default 7778)
      --tick HZ          Ticks a second (default 60)
      --max-clients N    Clients at once (default 64)
      --protocol N       The game's protocol number clients must match (default 0)
      --discovery PORT|off  UDP port to answer LAN probes on (default 7779)
      --name NAME        What LAN searches list the server as (default \"Voltra server\")
      --game ID          The game id LAN probes are matched against
      --lobby            Hold a lobby before the match (a game's server may already)
      --min-players N    The fewest players its match starts with (implies --lobby)
      --max-players N    The lobby's slots (implies --lobby)
      --seconds N        Stop after N seconds (default: run until killed)
      --port-file FILE   Once listening, write `udp PORT`, `ws PORT`, `discovery PORT` lines here
  -h, --help             Print this message";

/// What the server was asked to do.
#[derive(Debug, PartialEq)]
pub enum Parsed {
    /// Host and run.
    Run(Box<Args>),
    /// Print [`USAGE`] and stop.
    Help,
}

/// A parsed command line.
#[derive(Debug, PartialEq)]
pub struct Args {
    /// The scene to run, an asset path; none runs an empty world.
    pub scene: Option<String>,
    /// Where asset paths resolve from; none takes the default root.
    pub asset_root: Option<PathBuf>,
    /// How to host and tick.
    pub options: ServerOptions,
    /// Stop after this long; none runs until killed.
    pub seconds: Option<Duration>,
    /// Where to write the ports once listening.
    pub port_file: Option<PathBuf>,
    /// The lobby's rules the command line changed — laid over the lobby a
    /// game's server sets in [`ServerOptions::lobby`] (or the engine's
    /// default one) once the game has had its say.
    pub lobby: LobbyFlags,
}

/// The lobby flags given, each `None` when not.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LobbyFlags {
    /// `--lobby`, or any other lobby flag: hold one.
    pub on: bool,
    /// `--min-players`.
    pub min_players: Option<usize>,
    /// `--max-players`.
    pub max_players: Option<usize>,
}

impl LobbyFlags {
    /// `options` with these flags laid over its lobby — the engine's
    /// default one when the flags ask for a lobby and it has none.
    pub fn apply(&self, options: &mut ServerOptions) {
        if !self.on {
            return;
        }
        let lobby = options.lobby.get_or_insert_with(LobbyConfig::default);
        if let Some(min) = self.min_players {
            lobby.min_players = min;
        }
        if let Some(max) = self.max_players {
            lobby.max_players = max;
        }
    }
}

/// Everything a command line can get wrong.
#[derive(Debug, PartialEq)]
pub enum ArgError {
    /// A flag given no value.
    MissingValue(&'static str),
    /// A flag given a value it cannot take.
    BadValue(&'static str, String),
    /// A second scene.
    ExtraScene(String),
    /// A flag nobody knows.
    UnknownFlag(String),
}

impl fmt::Display for ArgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingValue(flag) => write!(f, "`{flag}` needs a value"),
            Self::BadValue(flag, value) => write!(f, "`{flag}` cannot be `{value}`"),
            Self::ExtraScene(extra) => {
                write!(f, "only one scene can be run, and `{extra}` is a second")
            }
            Self::UnknownFlag(flag) => write!(f, "unknown option `{flag}`"),
        }
    }
}

/// Reads a command line, the program's name left out.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Parsed, ArgError> {
    let mut parsed = Args {
        scene: None,
        asset_root: None,
        options: ServerOptions {
            host: HostOptions {
                udp_port: Some(DEFAULT_UDP_PORT),
                websocket_port: Some(DEFAULT_WEBSOCKET_PORT),
                config: NetConfig::default(),
                discovery: Some(Advertise {
                    port: DEFAULT_DISCOVERY_PORT,
                    ..Advertise::default()
                }),
                ..HostOptions::default()
            },
            tick_rate: DEFAULT_TICK_RATE,
            lobby: None,
        },
        seconds: None,
        port_file: None,
        lobby: LobbyFlags::default(),
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = |flag: &'static str| args.next().ok_or(ArgError::MissingValue(flag));
        match arg.as_str() {
            "-h" | "--help" => return Ok(Parsed::Help),
            "--asset-root" => parsed.asset_root = Some(value("--asset-root")?.into()),
            "--bind" => {
                let ip = value("--bind")?;
                parsed.options.host.bind = ip
                    .parse::<IpAddr>()
                    .map_err(|_| ArgError::BadValue("--bind", ip))?;
            }
            "--udp" => parsed.options.host.udp_port = port("--udp", value("--udp")?)?,
            "--ws" => parsed.options.host.websocket_port = port("--ws", value("--ws")?)?,
            "--tick" => parsed.options.tick_rate = number("--tick", value("--tick")?)?,
            "--max-clients" => {
                parsed.options.host.config.max_clients =
                    number("--max-clients", value("--max-clients")?)?
            }
            "--protocol" => {
                parsed.options.host.config.protocol = number("--protocol", value("--protocol")?)?
            }
            "--seconds" => {
                let seconds: u64 = number("--seconds", value("--seconds")?)?;
                parsed.seconds = Some(Duration::from_secs(seconds));
            }
            "--port-file" => parsed.port_file = Some(value("--port-file")?.into()),
            "--discovery" => match port("--discovery", value("--discovery")?)? {
                Some(at) => {
                    parsed
                        .options
                        .host
                        .discovery
                        .get_or_insert_with(Advertise::default)
                        .port = at;
                }
                None => parsed.options.host.discovery = None,
            },
            "--name" => {
                let name = value("--name")?;
                if let Some(advertise) = parsed.options.host.discovery.as_mut() {
                    advertise.name = name;
                }
            }
            "--game" => {
                let game = value("--game")?;
                if let Some(advertise) = parsed.options.host.discovery.as_mut() {
                    advertise.game = game;
                }
            }
            "--lobby" => parsed.lobby.on = true,
            "--min-players" => {
                parsed.lobby.on = true;
                parsed.lobby.min_players = Some(number("--min-players", value("--min-players")?)?);
            }
            "--max-players" => {
                parsed.lobby.on = true;
                parsed.lobby.max_players = Some(number("--max-players", value("--max-players")?)?);
            }
            flag if flag.starts_with('-') => return Err(ArgError::UnknownFlag(arg)),
            _ if parsed.scene.is_some() => return Err(ArgError::ExtraScene(arg)),
            _ => parsed.scene = Some(arg),
        }
    }
    Ok(Parsed::Run(Box::new(parsed)))
}

fn port(flag: &'static str, value: String) -> Result<Option<u16>, ArgError> {
    if value == "off" {
        return Ok(None);
    }
    value
        .parse()
        .map(Some)
        .map_err(|_| ArgError::BadValue(flag, value))
}

fn number<T: std::str::FromStr + PartialOrd + Default>(
    flag: &'static str,
    value: String,
) -> Result<T, ArgError> {
    match value.parse::<T>() {
        Ok(n) if n > T::default() || flag == "--protocol" => Ok(n),
        _ => Err(ArgError::BadValue(flag, value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let Ok(Parsed::Run(args)) = run(&[
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
}
