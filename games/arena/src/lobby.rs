//! Before the match: the lobby screen a player hosts, finds and joins
//! from, and a bot's way through the lobby — ready at once, and the match
//! started as soon as it may be, when it hosts.

use std::time::Duration;

use voltra_core::lobby::{LobbyOutcome, LobbyScreen, LobbyScreenConfig};
use voltra_core::net::LobbyPhase;
use voltra_core::Tick;

use crate::session::{advertise, host_options, lobby_config, GAME_ID};

/// The screen the arena opens: hosting on the default ports, found on the
/// LAN as "<player>'s game", searching for arenas only.
pub fn screen_config() -> LobbyScreenConfig {
    let mut host = host_options(
        voltra_core::net::DEFAULT_UDP_PORT,
        voltra_core::net::DEFAULT_WEBSOCKET_PORT,
    );
    // Named after the player who hosts.
    host.discovery = Some(advertise(""));
    LobbyScreenConfig {
        host,
        lobby: lobby_config(),
        ..LobbyScreenConfig::for_game(GAME_ID)
    }
}

/// The lobby screen while it shows.
#[derive(Debug, Default)]
pub(crate) struct Screen(Option<LobbyScreen>);

impl Screen {
    /// Opens the screen: on its menu with no session, on the room with one.
    pub(crate) fn open(&mut self, tick: &mut Tick<'_>) {
        match LobbyScreen::open(tick, &screen_config()) {
            Ok(screen) => self.0 = Some(screen),
            Err(error) => log::error!("arena: no lobby screen: {error}"),
        }
    }

    /// One frame of it; taken away once the match starts.
    pub(crate) fn update(&mut self, tick: &mut Tick<'_>) {
        let Some(screen) = self.0.as_mut() else {
            return;
        };
        match screen.update(tick) {
            LobbyOutcome::Open => {}
            LobbyOutcome::Started => {
                screen.close(tick);
                self.0 = None;
            }
            LobbyOutcome::Closed => self.0 = None,
        }
    }
}

/// How long a bot waits for the server to start the match it asked for
/// before asking again (someone may have stopped being ready between).
const ASK_AGAIN: Duration = Duration::from_secs(1);

/// A bot in a lobby: ready as soon as it is in, and — hosting — the
/// match started as soon as it may be. A server with no player of its own
/// does nothing here: the bots that join it start it.
#[derive(Debug, Default)]
pub(crate) struct Bot {
    /// When it last asked to start.
    asked: Option<Duration>,
}

impl Bot {
    /// Per frame.
    pub(crate) fn update(&mut self, tick: &mut Tick<'_>) {
        let now = tick.net.now();
        let Some(lobby) = tick.net.lobby_mut() else {
            return;
        };
        let Some(me) = lobby.local_player() else {
            return;
        };
        if lobby.lobby().phase() != LobbyPhase::Waiting {
            self.asked = None;
            return;
        }
        if lobby.is_in() && !lobby.is_ready() {
            if let Err(error) = lobby.set_ready(true) {
                log::warn!("arena: the bot cannot be ready: {error}");
            }
        }
        let waited = self
            .asked
            .is_none_or(|asked| now.saturating_sub(asked) >= ASK_AGAIN);
        if waited && lobby.is_host() && lobby.lobby().can_start(me).is_ok() {
            self.asked = Some(now);
            match lobby.start() {
                Ok(()) => log::info!("arena: {me} starts the match"),
                Err(error) => log::warn!("arena: the bot cannot start: {error}"),
            }
        }
    }
}
