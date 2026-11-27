//! What every end draws of the match: each player in its own colour, and
//! the scores along the top.

use voltra_core::net::PeerId;
use voltra_core::replication::NetOwner;
use voltra_ecs::World;
use voltra_scene::{Name, Sprite2D, Text};

use crate::{Player, Round, Score};

/// A colour per player, by peer number.
const COLOURS: [[f32; 4]; 4] = [
    [0.35, 0.75, 1.0, 1.0],
    [1.0, 0.45, 0.35, 1.0],
    [0.5, 0.9, 0.4, 1.0],
    [0.85, 0.55, 1.0, 1.0],
];

fn label(peer: u32) -> String {
    if peer == PeerId::SERVER.0 {
        "Host".to_owned()
    } else {
        format!("P{peer}")
    }
}

/// Colours the players and writes the scores into the scene's `Scores`
/// text. Per frame, on every end, from replicated state only.
pub(crate) fn draw(world: &mut World) {
    let mut scores: Vec<(u32, u32)> = Vec::new();
    let players: Vec<_> = world
        .query::<NetOwner>()
        .filter(|(entity, _)| world.get::<Player>(*entity).is_some())
        .map(|(entity, owner)| (entity, owner.0 .0))
        .collect();
    for (entity, peer) in players {
        if let Some(sprite) = world.get_mut::<Sprite2D>(entity) {
            sprite.color = COLOURS[peer as usize % COLOURS.len()];
        }
        scores.push((peer, world.get::<Score>(entity).map_or(0, |s| s.0)));
    }
    scores.sort_unstable();
    let round = world
        .query::<Round>()
        .map(|(_, r)| *r)
        .next()
        .unwrap_or_default();
    let line = match round.winner {
        Some(peer) => format!("{} wins!", label(peer)),
        None => {
            let parts: Vec<String> = scores
                .iter()
                .map(|(peer, score)| format!("{} {score}", label(*peer)))
                .collect();
            format!("{}   — first to {}", parts.join("   "), round.target)
        }
    };
    let text = world
        .query::<Name>()
        .find(|(_, name)| name.as_str() == "Scores")
        .map(|(entity, _)| entity);
    if let Some(text) = text.and_then(|entity| world.get_mut::<Text>(entity)) {
        if text.content != line {
            text.content = line;
        }
    }
}
