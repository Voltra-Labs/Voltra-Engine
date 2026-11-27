//! The one way a player moves, on the server and in a client's prediction.

use serde::{Deserialize, Serialize};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::Transform2D;

use crate::Player;

/// A player's input for one step: a direction, each axis `-127..=127`,
/// and a zap fired this step, if any.
///
/// Whole numbers rather than floats: two bytes on the wire (four with a
/// zap), and the server and the client turn them into the same direction
/// exactly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Steer {
    /// Left (negative) to right.
    pub x: i8,
    /// Down (negative) to up.
    pub y: i8,
    /// The way a zap is fired this step, as `x` and `y` are: aimed where
    /// this player saw its target, which the server rewinds to.
    pub zap: Option<(i8, i8)>,
}

/// A direction as two whole numbers, `-127..=127` each.
fn whole(direction: Vec2) -> (i8, i8) {
    let direction = direction.clamp_length_max(1.0) * 127.0;
    (
        direction.x.round().clamp(-127.0, 127.0) as i8,
        direction.y.round().clamp(-127.0, 127.0) as i8,
    )
}

impl Steer {
    /// The input pointing along `direction` (any length; clamped to one).
    pub fn toward(direction: Vec2) -> Self {
        let (x, y) = whole(direction);
        Self { x, y, zap: None }
    }

    /// The same, firing a zap along `direction`.
    pub fn zapping(self, direction: Vec2) -> Self {
        Self {
            zap: Some(whole(direction.normalize_or_zero())),
            ..self
        }
    }

    /// The direction it points, at most one long.
    pub fn direction(&self) -> Vec2 {
        (Vec2::new(f32::from(self.x), f32::from(self.y)) / 127.0).clamp_length_max(1.0)
    }

    /// The way its zap goes, one long; `None` without one, or with one of
    /// no direction.
    pub fn zap_direction(&self) -> Option<Vec2> {
        let (x, y) = self.zap?;
        Vec2::new(f32::from(x), f32::from(y)).try_normalize()
    }
}

/// Moves `entity` one step of `step` seconds by `steer`, inside its
/// player's bounds.
///
/// Reads nothing but the entity's own [`Player`] and `Transform2D` (both
/// replicated), the input and the step, so a client's replay of it lands
/// where the server's run did. A zap moves nothing.
pub fn walk(world: &mut World, entity: Entity, steer: &Steer, step: f32) {
    let Some(player) = world.get::<Player>(entity).copied() else {
        return;
    };
    if let Some(transform) = world.get_mut::<Transform2D>(entity) {
        let moved = transform.translation + steer.direction() * player.speed * step;
        transform.translation = moved.clamp(-player.bounds, player.bounds);
    }
}
