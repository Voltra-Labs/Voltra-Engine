//! Arena: two to four players race for a coin and zap each other, first
//! to five — a networked game of its own.
//!
//! One game value for every end of a match. Where the match is decided —
//! the server, a listen server, or a run alone — it runs the rules
//! (`authority.rs`): a player per peer, each moved by its own input, the
//! coin, the zaps (`zap.rs`: each tested where its shooter saw its target,
//! by the engine's lag compensation), the round. A client only predicts
//! its own player ([`Prediction`]) and lets replication bring the rest: the players'
//! `Transform2D`, [`Player`] and [`Score`], the coin's place and the
//! [`Round`]. The same [`walk`] moves a player on both, which is what
//! makes the prediction land where the server says.
//!
//! Before the match, a lobby (`lobby.rs`, the engine's lobby screen):
//! players join, ready up, and the host starts it; the server then spawns
//! a player for each, and for any who drop in after.
//!
//! `cargo run -p arena` opens the lobby screen (host, find on the LAN,
//! join by address); `-- --host` hosts at once, `-- --join
//! udp://host:7777` joins, `-- --solo` plays alone; `cargo run -p arena
//! --bin arena-server` is a dedicated server; in the editor, `Window ▸
//! Network` plays it with clients (with no lobby: they play at once).

#![warn(missing_docs)]

mod authority;
mod components;
mod hud;
mod lobby;
mod movement;
mod pilot;
mod session;
mod zap;

pub use components::{Coin, Player, Round, Score};
pub use lobby::screen_config;
pub use movement::{walk, Steer};
pub use pilot::Pilot;
pub use session::{
    advertise, host_options, lobby_config, net_config, replication_config, Session, GAME_ID,
    INTEREST_RADIUS, PROTOCOL,
};
pub use zap::{ZAP_COOLDOWN, ZAP_RANGE};

use voltra_core::net::PeerId;
use voltra_core::replication::Prediction;
use voltra_core::{Game, Tick};
use voltra_scene::ComponentRegistry;

/// The first scene, under the game's asset root.
pub const FIRST_SCENE: &str = "scenes/main.ron";

/// The prefab every player is spawned from.
pub const PLAYER_PREFAB: &str = "prefabs/player.prefab.ron";

/// The game. Made afresh for every play in the editor.
#[derive(Default)]
pub struct Arena {
    session: Session,
    pilot: Pilot,
    prediction: Prediction<Steer>,
    dice: authority::Dice,
    cooldowns: zap::Cooldowns,
    screen: lobby::Screen,
    bot: lobby::Bot,
}

impl Arena {
    /// A run that plays `session`, steered by `pilot`.
    pub fn new(session: Session, pilot: Pilot) -> Self {
        Self {
            session,
            pilot,
            ..Self::default()
        }
    }

    /// What this run's player has predicted, on a client.
    pub fn prediction(&self) -> &Prediction<Steer> {
        &self.prediction
    }
}

/// Whether this end decides the match: a server, or a run with no session.
fn decides(tick: &Tick<'_>) -> bool {
    !tick.net.is_active() || tick.net.is_server()
}

impl Game for Arena {
    fn register(&self, registry: &mut ComponentRegistry) {
        registry
            .register::<Player>(Player::NAME)
            .with_template()
            .replicated();
        registry
            .register::<Score>(Score::NAME)
            .with_template()
            .replicated();
        registry.register::<Coin>(Coin::NAME).with_template();
        registry
            .register::<Round>(Round::NAME)
            .with_template()
            .replicated();
    }

    fn start(&mut self, tick: &mut Tick<'_>) {
        self.session.begin(tick);
        // Alone, this run is the only player; in a lobby, every player —
        // the host's too — is spawned when the match starts.
        if !tick.net.is_active() && self.session == Session::Solo {
            authority::spawn_player(tick, PeerId::SERVER);
        }
        // A player picks from the lobby screen; a bot finds its own way.
        if self.session != Session::Solo && self.pilot == Pilot::Keys {
            self.screen.open(tick);
        }
    }

    fn update(&mut self, tick: &mut Tick<'_>) {
        match self.pilot {
            Pilot::Keys => self.screen.update(tick),
            Pilot::Bot => self.bot.update(tick),
        }
        if decides(tick) {
            authority::follow_peers(tick);
        }
        hud::draw(tick.world);
    }

    fn fixed_update(&mut self, tick: &mut Tick<'_>) {
        if decides(tick) {
            authority::step(tick, self.pilot, &mut self.dice, &mut self.cooldowns);
            return;
        }
        // A client: its own player, ahead of the server.
        let Some(mine) = self.prediction.entity().or_else(|| {
            let local = tick.net.local_id()?;
            voltra_core::Net::owned_by(tick.world, local)
        }) else {
            return;
        };
        let steer = self.pilot.steer(tick.input, tick.world, mine);
        let before = self.prediction.entity();
        tick.predict(&mut self.prediction, steer, walk);
        if before.is_none() && self.prediction.entity().is_some() {
            if let Some(peer) = tick.net.local_id() {
                log::info!("arena: playing as {peer}, predicted");
            }
        }
    }
}

voltra_core::export_game!(Arena::default());
