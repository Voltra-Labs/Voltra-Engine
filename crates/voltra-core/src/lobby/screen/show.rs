//! The screen brought up to date with the session: the page, the status,
//! the lists, what may be pressed.

use voltra_ecs::Entity;
use voltra_net::{LobbyPhase, PeerId};
use voltra_scene::{UiButton, UiTextField, UiToggle};

use super::list::{hidden, set_words};
use super::{labels, LobbyScreen};
use crate::app::Tick;

impl LobbyScreen {
    /// The screen brought up to date: which page, the lists, the buttons
    /// only the host may press.
    pub(super) fn show(&mut self, tick: &mut Tick<'_>) {
        let in_session = tick.net.is_active();
        if let Some(menu) = self.menu {
            hidden(tick, menu, in_session);
        }
        if let Some(room) = self.room {
            hidden(tick, room, !in_session);
        }
        let status = self.status;
        say(tick, status, &self.said);

        self.found = tick.net.discovered().to_vec();
        let servers = self.found.iter().map(labels::server).collect();
        self.servers.show(tick, servers);

        let (rows, in_lobby, ready, startable) = match tick.net.lobby() {
            Some(session) => {
                let lobby = session.lobby();
                let me = session.local_player();
                let rows = lobby
                    .entries()
                    .iter()
                    .map(|entry| labels::player(entry, lobby.host(), me))
                    .collect();
                let startable =
                    session.is_host() && me.is_some_and(|me: PeerId| lobby.can_start(me).is_ok());
                (
                    rows,
                    session.is_in() && lobby.phase() == LobbyPhase::Waiting,
                    session.is_ready(),
                    startable,
                )
            }
            None => (Vec::new(), false, false, false),
        };
        self.players.show(tick, rows);
        if let Some(toggle) = self.ready {
            if let Some(t) = tick.world.get_mut::<UiToggle>(toggle) {
                if t.on != ready {
                    t.on = ready;
                }
            }
            if let Some(b) = tick.world.get_mut::<UiButton>(toggle) {
                b.disabled = !in_lobby;
            }
        }
        if let Some(b) = self.start.and_then(|e| tick.world.get_mut::<UiButton>(e)) {
            b.disabled = !startable;
        }
    }

    pub(super) fn field(&self, tick: &Tick<'_>, field: Option<Entity>) -> String {
        field
            .and_then(|e| tick.world.get::<UiTextField>(e))
            .map(|f| f.text.clone())
            .unwrap_or_default()
    }

    pub(super) fn set_field(&self, tick: &mut Tick<'_>, field: Option<Entity>, text: &str) {
        if let Some(f) = field.and_then(|e| tick.world.get_mut::<UiTextField>(e)) {
            f.text = text.to_owned();
            f.caret = f.text.chars().count();
            f.anchor = f.caret;
        }
    }
}

/// Sets the text of `label`, if there is one: as written, since a status
/// may carry an address or a reason from elsewhere.
fn say(tick: &mut Tick<'_>, label: Option<Entity>, words: &str) {
    set_words(tick.world, label, words);
}
