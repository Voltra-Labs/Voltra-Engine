//! The LAN discovery flags, laid over the discovery a game's server sets.

use crate::ServerOptions;
use voltra_core::net::{Advertise, DiscoveryScope};

/// The discovery flags given, each `None` when not. Read in any order, and
/// laid over [`HostOptions::discovery`](voltra_core::net::HostOptions::discovery)
/// once the game's server has set its own (its game id, its name): what
/// the command line says wins.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct DiscoveryFlags {
    /// `--discovery PORT` (`Some(Some(port))`) or `--discovery off`
    /// (`Some(None)`); the last one given.
    pub port: Option<Option<u16>>,
    /// `--name`.
    pub name: Option<String>,
    /// `--game`.
    pub game: Option<String>,
    /// `--discovery-public`: answer probes from any address, not only the
    /// local network's.
    pub public: bool,
}

impl DiscoveryFlags {
    /// `options` with these flags laid over its discovery. `--discovery
    /// off` turns it off whatever else was given (and says what that
    /// leaves unused); any other discovery flag turns it on — the engine's
    /// default advert — if the game had it off.
    pub fn apply(&self, options: &mut ServerOptions) {
        if self.port == Some(None) {
            if self.name.is_some() || self.game.is_some() || self.public {
                log::warn!(
                    "--discovery off: --name, --game and --discovery-public have nothing to change"
                );
            }
            options.host.discovery = None;
            return;
        }
        let asked =
            self.port.is_some() || self.name.is_some() || self.game.is_some() || self.public;
        let advertise = match options.host.discovery.as_mut() {
            Some(advertise) => advertise,
            None if asked => options.host.discovery.insert(Advertise::default()),
            None => return,
        };
        if let Some(Some(port)) = self.port {
            advertise.port = port;
        }
        if let Some(name) = &self.name {
            advertise.name.clone_from(name);
        }
        if let Some(game) = &self.game {
            advertise.game.clone_from(game);
        }
        if self.public {
            advertise.scope = DiscoveryScope::Anyone;
        }
    }
}
