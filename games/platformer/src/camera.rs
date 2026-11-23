//! The camera eased after the walker.

use voltra_core::Tick;
use voltra_ecs::Entity;
use voltra_scene::{Camera, Transform};

/// How much of the distance to the walker the camera closes per second.
///
/// A fraction rather than a speed: the camera eases in without overshooting,
/// and the frame rate cannot change where it ends up.
const FOLLOW: f32 = 6.0;

/// Eases every scene camera towards `target`, horizontally.
///
/// Per frame rather than per step, because this is what is looked at rather
/// than what is simulated — and because a camera that moved on the physics
/// clock would judder on any frame that owed two steps.
pub fn follow(tick: &mut Tick<'_>, target: Entity) {
    let Some(&Transform { translation, .. }) = tick.world.get::<Transform>(target) else {
        return;
    };
    let cameras: Vec<Entity> = tick.world.query::<Camera>().map(|(e, _)| e).collect();
    // Frame-rate independent easing: the fraction closed per second is the
    // constant, not the fraction closed per frame.
    let t = 1.0 - (-FOLLOW * tick.delta).exp();
    for camera in cameras {
        if let Some(transform) = tick.world.get_mut::<Transform>(camera) {
            transform.translation.x += (translation.x - transform.translation.x) * t;
        }
    }
}
