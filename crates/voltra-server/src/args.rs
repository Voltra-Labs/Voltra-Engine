//! The server's command line, which a game's own server binary shares
//! through [`crate::cli`].

use std::fmt;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

mod discovery_flags;
mod lobby_flags;

pub use discovery_flags::DiscoveryFlags;
pub use lobby_flags::LobbyFlags;

use crate::{ServerOptions, DEFAULT_TICK_RATE};
use voltra_core::net::{
    Advertise, HostOptions, NetConfig, DEFAULT_DISCOVERY_PORT, DEFAULT_UDP_PORT,
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
      --discovery-public Answer LAN probes from any address, not only private,
                         link-local and loopback ones (a broadcast never crosses
                         a router: only a stranger probes from elsewhere)
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
    /// The discovery flags the command line gave, laid over
    /// [`HostOptions::discovery`] the same way.
    pub discovery: DiscoveryFlags,
}

impl Args {
    /// Lays the command line's lobby and discovery flags over what the
    /// game's server set in [`options`](Self::options): call it once the
    /// game has had its say (`cli::main` does).
    pub fn apply_flags(&mut self) {
        let lobby = self.lobby.clone();
        lobby.apply(&mut self.options);
        let discovery = self.discovery.clone();
        discovery.apply(&mut self.options);
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
        discovery: DiscoveryFlags::default(),
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
            "--discovery" => {
                parsed.discovery.port = Some(port("--discovery", value("--discovery")?)?);
            }
            "--discovery-public" => parsed.discovery.public = true,
            "--name" => parsed.discovery.name = Some(value("--name")?),
            "--game" => parsed.discovery.game = Some(value("--game")?),
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
mod tests;
