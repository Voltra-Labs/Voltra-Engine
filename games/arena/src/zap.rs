//! A zap: a shot along a line at another player, worth a coin when it
//! lands — tested where the shooter saw its target, not where the target
//! has got to since.
//!
//! A client draws the other players a round trip and a jitter buffer
//! behind the server, so it aims at where they were. The server runs the
//! zap's ray in the world as that client saw it (`tick.net.rewound`), its
//! players' colliders put back to the moment the shooter's input says it
//! was looking at, within the engine's bound. The shooter's own place is
//! its own, now: it predicted it.

use std::collections::HashMap;

use voltra_core::net::PeerId;
use voltra_core::replication::NetOwner;
use voltra_core::{QueryFilter, Rewound, Tick};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::Transform;

use crate::Player;

/// How far a zap reaches, in world units.
pub const ZAP_RANGE: f32 = 6.0;

/// Seconds between one player's zaps.
pub const ZAP_COOLDOWN: f32 = 1.0;

/// Seconds until each player may zap again, on the server.
#[derive(Debug, Clone, Default)]
pub(crate) struct Cooldowns(HashMap<PeerId, f32>);

impl Cooldowns {
    /// A step of `delta` seconds goes by; a cooldown over is forgotten.
    pub(crate) fn tick(&mut self, delta: f32) {
        self.0.retain(|_, left| {
            *left -= delta;
            *left > 0.0
        });
    }

    /// Whether `peer` may zap, and if so starts its cooldown.
    fn take(&mut self, peer: PeerId) -> bool {
        if self.0.contains_key(&peer) {
            return false;
        }
        self.0.insert(peer, ZAP_COOLDOWN);
        true
    }
}

/// The nearest other player within reach, as `world` has it: where a zap
/// from `player` is aimed. `None` with nobody in reach.
pub(crate) fn aim(world: &World, player: Entity) -> Option<Vec2> {
    let from = world.get::<Transform>(player)?.translation;
    world
        .query::<Player>()
        .filter(|(other, _)| *other != player)
        .filter_map(|(other, _)| world.get::<Transform>(other))
        .map(|t| t.translation - from)
        .filter(|to| to.length() <= ZAP_RANGE && *to != Vec2::ZERO)
        .min_by(|a, b| a.length().total_cmp(&b.length()))
}

/// Fires `peer`'s zap from `shooter` along `direction`, if its cooldown
/// allows: answers the player it hit, in the world as `peer` saw it.
pub(crate) fn fire(
    tick: &Tick<'_>,
    shooter: Entity,
    peer: PeerId,
    direction: Vec2,
    cooldowns: &mut Cooldowns,
) -> Option<Entity> {
    if !cooldowns.take(peer) {
        return None;
    }
    let world: &World = tick.world;
    let from = world.get::<Transform>(shooter)?.translation;
    let filter = QueryFilter::new().excluding(shooter);
    let seen = tick.net.rewound(world, peer);
    let hit = seen.ray(from, direction, ZAP_RANGE, filter)?;
    world.get::<Player>(hit.entity)?;

    let newest = tick
        .net
        .pose_history()
        .and_then(|history| history.newest())
        .map(f64::from);
    let back = match (newest, seen.tick()) {
        (Some(newest), Some(then)) => newest - then,
        _ => 0.0,
    };
    let now = Rewound::live(world).ray(from, direction, ZAP_RANGE, filter);
    let target = world
        .get::<NetOwner>(hit.entity)
        .map_or_else(|| "a player".to_owned(), |owner| owner.0.to_string());
    log::info!(
        "arena: {peer} zaps {target}, {back:.1} ticks back{}",
        if now.map(|h| h.entity) == Some(hit.entity) {
            ""
        } else {
            " (where it is now, a miss)"
        }
    );
    Some(hit.entity)
}
