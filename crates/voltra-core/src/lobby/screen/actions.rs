//! What the screen's buttons do: host, look on the LAN, join; ready and
//! start in the lobby.

use voltra_net::SearchOptions;
use voltra_scene::UiToggle;

use super::{clicked, LobbyScreen};
use crate::app::Tick;

impl LobbyScreen {
    pub(super) fn menu_clicks(&mut self, tick: &mut Tick<'_>) {
        if clicked(tick, self.host) {
            self.host(tick);
        } else if clicked(tick, self.find) {
            self.find(tick);
        } else if clicked(tick, self.join) || self.address.is_some_and(|a| tick.ui.submitted(a)) {
            let address = self.field(tick, self.address);
            self.join(tick, &address);
        } else if let Some(row) = self.servers.clicked(tick) {
            if let Some(address) = self.found.get(row).and_then(|s| s.address()) {
                let address = address.to_owned();
                self.join(tick, &address);
            }
        }
    }

    pub(super) fn room_clicks(&mut self, tick: &mut Tick<'_>) {
        if let Some(toggle) = self.ready.filter(|&e| tick.ui.changed(e)) {
            let on = tick.world.get::<UiToggle>(toggle).is_some_and(|t| t.on);
            if let Some(Err(e)) = tick.net.lobby_mut().map(|lobby| lobby.set_ready(on)) {
                self.said = format!("Cannot be ready: {e}");
            }
        }
        if clicked(tick, self.start) {
            match tick.net.lobby_mut().map(|lobby| lobby.start()) {
                Some(Ok(())) => self.said = "Starting…".to_owned(),
                Some(Err(e)) => self.said = format!("Cannot start: {e}"),
                None => self.said = "Not in a lobby yet".to_owned(),
            }
        }
    }

    /// The player's name as typed, else the one the lobby was set up with.
    fn player_name(&self, tick: &Tick<'_>) -> String {
        let typed = self.field(tick, self.name);
        if typed.trim().is_empty() {
            self.config.lobby.name.clone()
        } else {
            typed
        }
    }

    fn host(&mut self, tick: &mut Tick<'_>) {
        let name = self.player_name(tick);
        tick.net.set_player_name(&name);
        let mut options = self.config.host.clone();
        options.listen = true;
        if let Some(advertise) = options.discovery.as_mut() {
            if advertise.name.is_empty() {
                advertise.name = format!("{name}'s game");
            }
            if advertise.game.is_empty() {
                advertise.game.clone_from(&self.config.game);
            }
        }
        self.said = match tick.net.host(&options) {
            Ok(()) => "Hosting: waiting for players".to_owned(),
            Err(e) => format!("Could not host: {e}"),
        };
    }

    fn find(&mut self, tick: &mut Tick<'_>) {
        let options = SearchOptions {
            game: self.config.game.clone(),
            fingerprint: tick
                .net
                .session_fingerprint(self.config.host.config.clone()),
            ..self.config.search.clone()
        };
        match tick.net.discover(options) {
            Ok(()) => {
                self.searching = true;
                self.said = "Looking for games on the LAN…".to_owned();
            }
            Err(e) => self.said = e.to_string(),
        }
    }

    fn join(&mut self, tick: &mut Tick<'_>, address: &str) {
        let name = self.player_name(tick);
        tick.net.set_player_name(&name);
        self.said = match tick
            .net
            .join(address.trim(), self.config.host.config.clone())
        {
            Ok(()) => format!("Joining {}…", address.trim()),
            Err(e) => format!("Could not join: {e}"),
        };
    }
}
