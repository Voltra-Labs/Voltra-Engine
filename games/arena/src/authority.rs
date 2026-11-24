//! The rules, run where the match is decided: on the server, or alone.
//!
//! A player per peer as it joins, gone as it leaves; each player moved by
//! its own input for the step; the coin taken by whoever reaches it, the
//! round won at the target, and started again after a pause.

use voltra_assets::AssetPath;
use voltra_core::net::{NetEvent, PeerId};
use voltra_core::replication::NetOwner;
use voltra_core::{Net, Tick};
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{hierarchy, Transform};

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
        .find_map(|(coin, _)| Some((coin, world.get::<Transform>(coin)?.translation)))
}

/// Spawns `peer`'s player from its prefab, owned by it.
pub(crate) fn spawn_player(tick: &mut Tick<'_>, peer: PeerId) {
    let path = AssetPath::new(PLAYER_PREFAB).expect("the prefab path is valid");
    let prefab = tick.prefab(&path);
    let at = Transform::from_translation(spawn_point(peer));
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
pub(crate) fn follow_peers(tick: &mut Tick<'_>) {
    for event in tick.net.events().to_vec() {
        match event {
            NetEvent::Connected(peer) => spawn_player(tick, peer),
            NetEvent::Disconnected { peer, .. } => {
                if let Some(player) = Net::owned_by(tick.world, peer) {
                    hierarchy::despawn_recursive(tick.world, player);
                    log::info!("arena: {peer} left");
                }
            }
        }
    }
}

/// One fixed step of the rules. `pilot` steers this run's own player, the
/// host's.
pub(crate) fn step(tick: &mut Tick<'_>, pilot: Pilot, dice: &mut Dice) {
    let players: Vec<(Entity, PeerId)> = tick
        .world
        .query::<NetOwner>()
        .filter(|(entity, _)| tick.world.get::<Player>(*entity).is_some())
        .map(|(entity, owner)| (entity, owner.0))
        .collect();
    for &(entity, peer) in &players {
        let steer: Option<Steer> = if peer == PeerId::SERVER {
            Some(pilot.steer(tick.input, tick.world, entity))
        } else {
            // None in time: the client predicted no step either.
            tick.net.input(peer)
        };
        if let Some(steer) = steer {
            walk(tick.world, entity, &steer, tick.delta);
        }
    }

    let Some((round_entity, mut round)) = tick
        .world
        .query::<Round>()
        .map(|(entity, round)| (entity, *round))
        .next()
    else {
        return;
    };
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
                .get::<Transform>(entity)
                .is_some_and(|t| t.translation.distance(at) < REACH)
        });
        if let Some((entity, peer)) = taker {
            let score = tick.world.get::<Score>(entity).map_or(0, |s| s.0) + 1;
            tick.world.insert(entity, Score(score));
            let bounds = tick
                .world
                .get::<Player>(entity)
                .map_or(Vec2::splat(3.0), |p| p.bounds);
            if let Some(transform) = tick.world.get_mut::<Transform>(coin) {
                transform.translation = Vec2::new(
                    (dice.unit() * 2.0 - 1.0) * bounds.x * 0.9,
                    (dice.unit() * 2.0 - 1.0) * bounds.y * 0.9,
                );
            }
            if score >= round.target {
                round.winner = Some(peer.0);
                round.left = round.pause;
                log::info!("arena: {peer} wins round {}", round.finished + 1);
            }
        }
    }
    tick.world.insert(round_entity, round);
}
