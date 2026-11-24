//! The one way a player moves, on the server and in a client's prediction.

use serde::{Deserialize, Serialize};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::Transform;

use crate::Player;

/// A player's input for one step: a direction, each axis `-127..=127`.
///
/// Whole numbers rather than floats: two bytes on the wire, and the
/// server and the client turn them into the same direction exactly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Steer {
    /// Left (negative) to right.
    pub x: i8,
    /// Down (negative) to up.
    pub y: i8,
}

impl Steer {
    /// The input pointing along `direction` (any length; clamped to one).
    pub fn toward(direction: Vec2) -> Self {
        let direction = direction.clamp_length_max(1.0) * 127.0;
        Self {
            x: direction.x.round().clamp(-127.0, 127.0) as i8,
            y: direction.y.round().clamp(-127.0, 127.0) as i8,
        }
    }

    /// The direction it points, at most one long.
    pub fn direction(&self) -> Vec2 {
        (Vec2::new(f32::from(self.x), f32::from(self.y)) / 127.0).clamp_length_max(1.0)
    }
}

/// Moves `entity` one step of `step` seconds by `steer`, inside its
/// player's bounds.
///
/// Reads nothing but the entity's own [`Player`] and `Transform` (both
/// replicated), the input and the step, so a client's replay of it lands
/// where the server's run did.
pub fn walk(world: &mut World, entity: Entity, steer: &Steer, step: f32) {
    let Some(player) = world.get::<Player>(entity).copied() else {
        return;
    };
    if let Some(transform) = world.get_mut::<Transform>(entity) {
        let moved = transform.translation + steer.direction() * player.speed * step;
        transform.translation = moved.clamp(-player.bounds, player.bounds);
    }
}
