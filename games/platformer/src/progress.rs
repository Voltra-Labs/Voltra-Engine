//! What F5 keeps and F9 puts back: the game's own type in a save slot.

use serde::{Deserialize, Serialize};
use voltra_core::Tick;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::Transform;

/// The engine stores it in a slot without knowing what is in it.
#[derive(Debug, Serialize, Deserialize)]
struct Progress {
    coins: u32,
    position: (f32, f32),
}

/// The one slot this game uses.
const SLOT: &str = "quicksave";

/// Keeps the walker's position and the coin count in the save slot; logs and keeps nothing if the walker has no transform or the write fails.
pub fn save(tick: &mut Tick<'_>, walker: Entity, coins: u32) {
    let Some(at) = tick.world.get::<Transform>(walker).map(|t| t.translation) else {
        return;
    };
    let progress = Progress {
        coins,
        position: (at.x, at.y),
    };
    if let Err(e) = tick.saves.save(SLOT, &progress) {
        log::error!("could not save: {e}");
    }
}

/// Puts the walker back where the save left it and answers the coin count,
/// or `None` with nothing saved — or a save this version cannot read, which
/// is said rather than silently ignored.
pub fn load(tick: &mut Tick<'_>, walker: Entity) -> Option<u32> {
    let progress = match tick.saves.load::<Progress>(SLOT) {
        Ok(Some(progress)) => progress,
        Ok(None) => {
            log::info!("nothing saved yet; F5 saves");
            return None;
        }
        Err(e) => {
            log::error!("could not load: {e}");
            return None;
        }
    };
    let at = Vec2::new(progress.position.0, progress.position.1);
    crate::walker::place(tick.world, walker, at);
    Some(progress.coins)
}
