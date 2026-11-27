//! Who steers this run's player: the keys, or a bot chasing the coin —
//! either zapping at the nearest other player it sees.

use voltra_core::Input;
use voltra_ecs::{Entity, World};
use voltra_scene::Transform;

use crate::{zap, Coin, Steer};

/// Who steers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pilot {
    /// The move actions of `assets/default.input.ron`; `zap` zaps.
    #[default]
    Keys,
    /// Straight at the coin, zapping whoever is in reach: what a test, or
    /// a server short of players, plays with.
    Bot,
}

impl Pilot {
    /// This step's input for `player`, as this run's world has it.
    ///
    /// A zap is aimed at the nearest other player where this world draws
    /// it — the server rewinds to that moment to test it.
    pub(crate) fn steer(&self, input: &Input, world: &World, player: Entity) -> Steer {
        let steer = self.walk(input, world, player);
        let zapping = match self {
            Self::Keys => input.action("zap").is_held(),
            Self::Bot => true,
        };
        match zap::aim(world, player) {
            Some(direction) if zapping => steer.zapping(direction),
            _ => steer,
        }
    }

    fn walk(&self, input: &Input, world: &World, player: Entity) -> Steer {
        match self {
            Self::Keys => Steer::toward(voltra_render::glam::Vec2::new(
                input.action_axis("move_left", "move_right"),
                input.action_axis("move_down", "move_up"),
            )),
            Self::Bot => {
                let at = world.get::<Transform>(player).map(|t| t.translation);
                let coin = world
                    .query::<Coin>()
                    .find_map(|(coin, _)| world.get::<Transform>(coin))
                    .map(|t| t.translation);
                match (at, coin) {
                    (Some(at), Some(coin)) if at.distance(coin) > 0.05 => {
                        Steer::toward((coin - at).normalize_or_zero())
                    }
                    _ => Steer::default(),
                }
            }
        }
    }
}
