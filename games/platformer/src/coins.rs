//! Coins: taken when the walker touches one, counted on the HUD.

use voltra_core::{PlayParams, Tick, Touch};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{AudioSource, ParticleEmitter, Text, Transform};

use crate::components::{Coin, CoinCounter};

/// What every coin in `world` is worth together: what the counter can
/// reach.
pub fn total(world: &World) -> u32 {
    world.query::<Coin>().map(|(_, coin)| coin.value).sum()
}

/// Takes every coin `walker` began touching this step, and answers what
/// they were worth.
///
/// In the fixed tick because that is where an event is handed over exactly
/// once. Read per frame, the same `Began` would arrive on every frame that
/// owed no step, and a coin would be worth three of itself.
pub fn take(tick: &mut Tick<'_>, walker: Entity) -> u32 {
    let taken: Vec<(Entity, u32)> = tick
        .events
        .iter()
        .filter(|event| event.touch == Touch::Began)
        .filter_map(|event| event.other(walker))
        .filter_map(|other| {
            tick.world
                .get::<Coin>(other)
                .map(|coin| (other, coin.value))
        })
        .collect();

    let mut worth = 0;
    for (coin, value) in taken {
        worth += value;
        // Played before the despawn: the sound outlives the entity that named
        // it, because a one-shot from the tick is not tied to a source the
        // loop is following.
        if let Some(source) = tick.world.get::<AudioSource>(coin) {
            if let Some(clip) = source.clip_handle {
                // On the source's own bus, so a game's effects slider reaches
                // the pickup the same way it reaches everything else.
                let bus = tick.audio.bus(&source.bus);
                tick.audio.play_with(
                    clip,
                    PlayParams {
                        bus,
                        ..PlayParams::default()
                    },
                );
            }
        }
        if let Some(at) = tick.world.get::<Transform>(coin).map(|t| t.translation) {
            sparks(tick, at);
        }
        // Despawned mid-overlap on purpose: the pair ends by disappearing, and
        // the `Ended` event that follows is what a game would close a door on.
        tick.world.despawn(coin);
        log::info!("coin taken, worth {value}");
    }
    worth
}

/// A spray of sparks at `at`: a one-shot emitter, which the loop despawns
/// once the last spark has faded.
fn sparks(tick: &mut Tick<'_>, at: Vec2) {
    let sparks = tick.world.spawn();
    tick.world.insert(sparks, Transform::from_translation(at));
    let mut emitter = ParticleEmitter {
        emitting: false,
        one_shot: true,
        lifetime: (0.3, 0.6),
        speed: (1.5, 3.0),
        spread: std::f32::consts::TAU,
        acceleration: Vec2::new(0.0, -6.0),
        size: (0.15, 0.0),
        color_start: [1.0, 0.9, 0.3, 1.0],
        color_end: [1.0, 0.5, 0.1, 0.0],
        sort_order: 12,
        ..ParticleEmitter::default()
    };
    emitter.burst(18);
    tick.world.insert(sparks, emitter);
}

/// The counter's words for `taken` of `total`.
pub fn label(taken: u32, total: u32) -> String {
    format!("Coins {taken} / {total}")
}

/// Writes the count on every [`CoinCounter`].
pub fn show(world: &mut World, taken: u32, total: u32) {
    let counters: Vec<Entity> = world.query::<CoinCounter>().map(|(e, _)| e).collect();
    for counter in counters {
        if let Some(text) = world.get_mut::<Text>(counter) {
            text.content = label(taken, total);
        }
    }
}
