//! The lobby screen while it is open: spawned from its prefab, driving
//! `tick.net` from its buttons and showing the session and its lobby.

mod actions;
mod config;
mod labels;
mod list;
mod show;

pub use config::{LobbyOutcome, LobbyScreenConfig, LobbyScreenError};

use voltra_assets::AssetPath;
use voltra_ecs::Entity;
use voltra_net::{DisconnectReason, LobbyEvent, NetEvent, ServerInfo};
use voltra_scene::{hierarchy, PrefabInstance, PrefabPart, Transform, UiButton};

use crate::app::Tick;
use crate::prefab_parts::Parts;
use list::List;

/// The action that backs out: Escape, or a pad's East, in the default map.
const CANCEL: &str = "ui_cancel";

/// The lobby screen, open: the entities it spawned and what they stand
/// for. Hold one while it shows, call [`update`](Self::update) every frame
/// from the per-frame tick, and [`close`](Self::close) it when that
/// answers [`LobbyOutcome::Started`] (the match is the game's from there)
/// — or drop it when it answers [`LobbyOutcome::Closed`].
///
/// Outside a session it shows its menu — the name, Host, Find on LAN and
/// what was found, Join by address; in one, the room — the players, Ready,
/// Start (the host's alone), Leave.
#[derive(Debug)]
pub struct LobbyScreen {
    root: Entity,
    menu: Option<Entity>,
    room: Option<Entity>,
    status: Option<Entity>,
    name: Option<Entity>,
    host: Option<Entity>,
    find: Option<Entity>,
    address: Option<Entity>,
    join: Option<Entity>,
    ready: Option<Entity>,
    start: Option<Entity>,
    leave: Option<Entity>,
    servers: List,
    players: List,
    /// The servers the list shows, in its order.
    found: Vec<ServerInfo>,
    config: LobbyScreenConfig,
    /// What the status line says, kept until something new happens.
    said: String,
    /// Whether a search this screen started is still to report.
    searching: bool,
}

impl LobbyScreen {
    /// Spawns the screen over whatever is showing — the lobby turned on
    /// first if the game has not — and focuses its first control.
    pub fn open(tick: &mut Tick<'_>, config: &LobbyScreenConfig) -> Result<Self, LobbyScreenError> {
        let path = AssetPath::new(&config.prefab)
            .map_err(|_| LobbyScreenError::BadPath(config.prefab.clone()))?;
        let prefab = tick.prefab(&path);
        let instance = tick
            .spawn(prefab, &Transform::default())
            .map_err(LobbyScreenError::Spawn)?;
        let root = instance
            .root()
            .ok_or_else(|| LobbyScreenError::Empty(config.prefab.clone()))?;
        // The screen is the code's from here on: a prefab sync must not put
        // back the texts it writes.
        for &entity in &instance.entities {
            tick.world.remove::<PrefabInstance>(entity);
            tick.world.remove::<PrefabPart>(entity);
        }
        if !tick.net.has_lobby() {
            tick.net.enable_lobby(config.lobby.clone());
        }
        let parts = Parts::of(tick.world, &instance.entities, "lobby");
        let servers = List::new(
            tick,
            parts.get("Servers"),
            parts.get("Server row"),
            "Server label",
            "Server",
        );
        let players = List::new(
            tick,
            parts.get("Players"),
            parts.get("Player row"),
            "Player label",
            "Player",
        );
        let mut screen = Self {
            root,
            menu: parts.get("Lobby menu"),
            room: parts.get("Lobby room"),
            status: parts.get("Status"),
            name: parts.get("Name"),
            host: parts.get("Host"),
            find: parts.get("Find on LAN"),
            address: parts.get("Address"),
            join: parts.get("Join"),
            ready: parts.get("Ready"),
            start: parts.get("Start"),
            leave: parts.get("Leave"),
            servers,
            players,
            found: Vec::new(),
            config: config.clone(),
            said: String::new(),
            searching: false,
        };
        let name = tick
            .net
            .lobby()
            .map_or_else(|| config.lobby.name.clone(), |l| l.name().to_owned());
        screen.set_field(tick, screen.name, &name);
        screen.set_field(tick, screen.address, &config.address);
        if cfg!(target_arch = "wasm32") {
            // A browser can neither listen nor broadcast.
            for button in [screen.host, screen.find].into_iter().flatten() {
                if let Some(button) = tick.world.get_mut::<UiButton>(button) {
                    button.disabled = true;
                }
            }
            screen.said = "In a browser, join a game by its address".to_owned();
        }
        screen.show(tick);
        tick.ui.focus(screen.name.or(screen.join));
        Ok(screen)
    }

    /// The screen's root entity: everything it spawned is under it.
    pub fn root(&self) -> Entity {
        self.root
    }

    /// What the status line says now.
    pub fn status(&self) -> &str {
        &self.said
    }

    /// One frame: what the player pressed, carried out, and the session as
    /// it is now shown. Answers [`LobbyOutcome::Started`] in the frame the
    /// match starts.
    pub fn update(&mut self, tick: &mut Tick<'_>) -> LobbyOutcome {
        if !tick.world.is_alive(self.root) {
            log::warn!("the lobby screen was despawned under it; closing");
            return LobbyOutcome::Closed;
        }
        let mut outcome = LobbyOutcome::Open;
        for event in tick.net.lobby_events() {
            match event {
                LobbyEvent::Started => {
                    self.said = "The match is starting".to_owned();
                    outcome = LobbyOutcome::Started;
                }
                LobbyEvent::Refused(why) => self.said = format!("Not let in: {why}"),
                LobbyEvent::Finished => self.said = "Back in the lobby".to_owned(),
                _ => {}
            }
        }
        if !tick.net.is_server() {
            for event in tick.net.events() {
                match event {
                    NetEvent::Connected(_) => self.said = "Joined".to_owned(),
                    NetEvent::Disconnected { reason, .. } => {
                        if *reason != DisconnectReason::Left {
                            self.said = format!("Disconnected: {reason}");
                        }
                    }
                }
            }
        }
        if let Some(name) = self.name.filter(|&e| tick.ui.changed(e)) {
            let typed = self.field(tick, Some(name));
            tick.net.set_player_name(&typed);
        }
        let in_session = tick.net.is_active();
        let cancel = tick.input.action(CANCEL).pressed() && !tick.ui.cancelled();
        if in_session {
            if clicked(tick, self.leave) || cancel {
                tick.net.stop();
                self.said = "Left the game".to_owned();
            } else {
                self.room_clicks(tick);
            }
        } else {
            if cancel {
                self.close(tick);
                return LobbyOutcome::Closed;
            }
            self.menu_clicks(tick);
        }
        if self.searching && !tick.net.is_discovering() {
            self.searching = false;
            self.said = match tick.net.discovered().len() {
                0 => "No games found on the LAN".to_owned(),
                1 => "Found a game".to_owned(),
                n => format!("Found {n} games"),
            };
        }
        self.show(tick);
        outcome
    }

    /// Takes the screen away. The session — and the match it started —
    /// goes on.
    pub fn close(&mut self, tick: &mut Tick<'_>) {
        if tick.world.is_alive(self.root) {
            hierarchy::despawn_recursive(tick.world, self.root);
        }
        tick.ui.focus(None);
    }

    /// The rows of the server list, for a test or a game that drives it.
    pub fn server_rows(&self) -> &[Entity] {
        self.servers.rows()
    }

    /// The rows of the player list.
    pub fn player_rows(&self) -> &[Entity] {
        self.players.rows()
    }
}

/// Whether `control`, if the screen has it, was clicked this frame.
fn clicked(tick: &Tick<'_>, control: Option<Entity>) -> bool {
    control.is_some_and(|e| tick.ui.clicked(e))
}

#[cfg(test)]
mod tests;
