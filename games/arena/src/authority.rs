//! The rules, run where the match is decided: on the server, or alone.
//!
//! A player per peer as it joins, gone as it leaves; each player moved by
//! its own input for the step; a zap that lands, and the coin taken by
//! whoever reaches it, worth a point each; the round won at the target,
//! and started again after a pause.

use voltra_assets::AssetPath;
use voltra_core::net::{LobbyEvent, LobbyPhase, NetEvent, PeerId};
use voltra_core::replication::NetOwner;
use voltra_core::{Net, Tick};
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{hierarchy, Transform2D};

use crate::zap::{self, Cooldowns};
use crate::{walk, Coin, Pilot, Player, Round, Score, Steer, PLAYER_PREFAB};

/// How close a player must come to take the coin.
const REACH: f32 = 0.6;

/// Where the n-th player starts: a corner each.
fn spawn_point(peer: PeerId) -> Vec2 {
    const CORNERS: [Vec2; 4] = [
        Vec2::new(-6.0, 3.0),
        Vec2::new(6.0, -3.0),
        Vec2::new(6.0, 3.0),
        Vec2::new(-6.0, -3.0),
    ];
    CORNERS[peer.0 as usize % CORNERS.len()]
}

/// The server's dice for where the coin goes next: seeded, so a match
/// replays.
#[derive(Debug, Clone)]
pub(crate) struct Dice(u64);

impl Default for Dice {
    fn default() -> Self {
        Self(0x2545_F491_4F6C_DD1D)
    }
}

impl Dice {
    fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// The coin, and where it is.
fn coin_at(world: &voltra_ecs::World) -> Option<(Entity, Vec2)> {
    world
        .query::<Coin>()
        .find_map(|(coin, _)| Some((coin, world.get::<Transform2D>(coin)?.translation)))
}

/// Spawns `peer`'s player from its prefab, owned by it.
pub(crate) fn spawn_player(tick: &mut Tick<'_>, peer: PeerId) {
    let path = AssetPath::new(PLAYER_PREFAB).expect("the prefab path is valid");
    let prefab = tick.prefab(&path);
    let at = Transform2D::from_translation(spawn_point(peer));
    match tick.spawn(prefab, &at) {
        Ok(instance) => match instance.root() {
            Some(root) => {
                tick.world.insert(root, NetOwner(peer));
                log::info!("arena: {peer} plays");
            }
            None => log::error!("arena: {PLAYER_PREFAB} spawned nothing"),
        },
        Err(error) => log::error!("arena: no player for {peer}: {error}"),
    }
}

/// Joins and leaves: a player made or taken away. Per frame.
///
/// With a lobby, a player is made for each of its players when the match
/// starts, and for one joining it under way; without one — a session
/// started by someone else, the editor's network play — for each peer as
/// it connects.
pub(crate) fn follow_peers(tick: &mut Tick<'_>) {
    if let Some(lobby) = tick.net.lobby() {
        let playing = lobby.lobby().phase() == LobbyPhase::Playing;
        let everyone: Vec<PeerId> = lobby.lobby().entries().iter().map(|e| e.peer).collect();
        for event in tick.net.lobby_events().to_vec() {
            match event {
                LobbyEvent::Started => {
                    log::info!("arena: the match starts with {} players", everyone.len());
                    for &peer in &everyone {
                        spawn_once(tick, peer);
                    }
                }
                LobbyEvent::Joined { peer, .. } if playing => spawn_once(tick, peer),
                LobbyEvent::Left { peer } => despawn(tick, peer),
                _ => {}
            }
        }
        return;
    }
    for event in tick.net.events().to_vec() {
        match event {
            NetEvent::Connected(peer) => spawn_player(tick, peer),
            NetEvent::Disconnected { peer, .. } => despawn(tick, peer),
        }
    }
}

/// `peer`'s player, unless it has one.
fn spawn_once(tick: &mut Tick<'_>, peer: PeerId) {
    if Net::owned_by(tick.world, peer).is_none() {
        spawn_player(tick, peer);
    }
}

/// Takes `peer`'s player away, if it has one.
fn despawn(tick: &mut Tick<'_>, peer: PeerId) {
    if let Some(player) = Net::owned_by(tick.world, peer) {
        hierarchy::despawn_recursive(tick.world, player);
        log::info!("arena: {peer} left");
    }
}

/// One fixed step of the rules. `pilot` steers this run's own player, the
/// host's.
pub(crate) fn step(tick: &mut Tick<'_>, pilot: Pilot, dice: &mut Dice, cooldowns: &mut Cooldowns) {
    let players: Vec<(Entity, PeerId)> = tick
        .world
        .query::<NetOwner>()
        .filter(|(entity, _)| tick.world.get::<Player>(*entity).is_some())
        .map(|(entity, owner)| (entity, owner.0))
        .collect();
    let steers: Vec<(Entity, PeerId, Steer)> = players
        .iter()
        .filter_map(|&(entity, peer)| {
            let steer: Option<Steer> = if peer == PeerId::SERVER {
                Some(pilot.steer(tick.input, tick.world, entity))
            } else {
                // None in time: the client predicted no step either.
                tick.net.input(peer)
            };
            Some((entity, peer, steer?))
        })
        .collect();
    cooldowns.tick(tick.delta);

    let Some((round_entity, mut round)) = tick
        .world
        .query::<Round>()
        .map(|(entity, round)| (entity, *round))
        .next()
    else {
        move_all(tick, &steers);
        return;
    };
    // Zaps from where each shooter stood when it fired — before this
    // step's walk, as its client aimed — into the world as it saw it.
    let mut scorers: Vec<(Entity, PeerId)> = Vec::new();
    if round.winner.is_none() {
        for &(entity, peer, steer) in &steers {
            let landed = steer
                .zap_direction()
                .and_then(|direction| zap::fire(tick, entity, peer, direction, cooldowns));
            if landed.is_some() {
                scorers.push((entity, peer));
            }
        }
    }
    move_all(tick, &steers);

    if round.winner.is_some() {
        round.left -= tick.delta;
        if round.left <= 0.0 {
            round.winner = None;
            round.finished += 1;
            for &(entity, _) in &players {
                tick.world.insert(entity, Score(0));
            }
        }
    } else if let Some((coin, at)) = coin_at(tick.world) {
        let taker = players.iter().copied().find(|&(entity, _)| {
            tick.world
                .get::<Transform2D>(entity)
                .is_some_and(|t| t.translation.distance(at) < REACH)
        });
        if let Some((entity, peer)) = taker {
            scorers.push((entity, peer));
            let bounds = tick
                .world
                .get::<Player>(entity)
                .map_or(Vec2::splat(3.0), |p| p.bounds);
            if let Some(transform) = tick.world.get_mut::<Transform2D>(coin) {
                transform.translation = Vec2::new(
                    (dice.unit() * 2.0 - 1.0) * bounds.x * 0.9,
                    (dice.unit() * 2.0 - 1.0) * bounds.y * 0.9,
                );
            }
        }
    }
    for (entity, peer) in scorers {
        if round.winner.is_some() {
            break;
        }
        let score = tick.world.get::<Score>(entity).map_or(0, |s| s.0) + 1;
        tick.world.insert(entity, Score(score));
        if score >= round.target {
            round.winner = Some(peer.0);
            round.left = round.pause;
            log::info!("arena: {peer} wins round {}", round.finished + 1);
        }
    }
    tick.world.insert(round_entity, round);
}

/// Each player one step by its own input.
fn move_all(tick: &mut Tick<'_>, steers: &[(Entity, PeerId, Steer)]) {
    for (entity, _, steer) in steers {
        walk(tick.world, *entity, steer, tick.delta);
    }
}
