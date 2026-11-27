//! Arena: two to four players race for a coin and zap each other, first
//! to five — a networked game of its own.
//!
//! One game value for every end of a match. Where the match is decided —
//! the server, a listen server, or a run alone — it runs the rules
//! (`authority.rs`): a player per peer, each moved by its own input, the
//! coin, the zaps (`zap.rs`: each tested where its shooter saw its target,
//! by the engine's lag compensation), the round. A client only predicts
//! its own player ([`Prediction`]) and lets replication bring the rest: the players'
//! `Transform`, [`Player`] and [`Score`], the coin's place and the
//! [`Round`]. The same [`walk`] moves a player on both, which is what
//! makes the prediction land where the server says.
//!
//! `cargo run -p arena -- --host` hosts, `-- --join udp://host:7777` joins;
//! `cargo run -p arena --bin arena-server` is a dedicated server; in the
//! editor, `Window ▸ Network` plays it with clients.

#![warn(missing_docs)]

mod authority;
mod components;
mod hud;
mod movement;
mod pilot;
mod session;
mod zap;

pub use components::{Coin, Player, Round, Score};
pub use movement::{walk, Steer};
pub use pilot::Pilot;
pub use session::{net_config, replication_config, Session, INTEREST_RADIUS, PROTOCOL};
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
            .register::<Player>("Player")
            .with_template()
            .replicated();
        registry
            .register::<Score>("Score")
            .with_template()
            .replicated();
        registry.register::<Coin>("Coin").with_template();
        registry
            .register::<Round>("Round")
            .with_template()
            .replicated();
    }

    fn start(&mut self, tick: &mut Tick<'_>) {
        self.session.begin(tick);
        // Alone, this run is the only player; hosting, its player joins as
        // the session's first event, like every other.
        if !tick.net.is_active() {
            authority::spawn_player(tick, PeerId::SERVER);
        }
    }

    fn update(&mut self, tick: &mut Tick<'_>) {
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
