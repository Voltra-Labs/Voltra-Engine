//! The server's command line.

use std::fmt;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

use voltra_core::net::{HostOptions, NetConfig, DEFAULT_UDP_PORT, DEFAULT_WEBSOCKET_PORT};
use voltra_server::{ServerOptions, DEFAULT_TICK_RATE};

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
      --seconds N        Stop after N seconds (default: run until killed)
      --port-file FILE   Once listening, write `udp PORT` and `ws PORT` lines here
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
    pub scene: Option<String>,
    pub asset_root: Option<PathBuf>,
    pub options: ServerOptions,
    pub seconds: Option<Duration>,
    pub port_file: Option<PathBuf>,
}

/// Everything a command line can get wrong.
#[derive(Debug, PartialEq)]
pub enum ArgError {
    MissingValue(&'static str),
    BadValue(&'static str, String),
    ExtraScene(String),
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

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Parsed, ArgError> {
    let mut parsed = Args {
        scene: None,
        asset_root: None,
        options: ServerOptions {
            host: HostOptions {
                udp_port: Some(DEFAULT_UDP_PORT),
                websocket_port: Some(DEFAULT_WEBSOCKET_PORT),
                config: NetConfig::default(),
                ..HostOptions::default()
            },
            tick_rate: DEFAULT_TICK_RATE,
        },
        seconds: None,
        port_file: None,
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
        assert_eq!(args.scene, None);
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
