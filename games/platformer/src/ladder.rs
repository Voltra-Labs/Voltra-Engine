//! Ladders: tiles whose frame says `ladder: true` in the tileset. Over one,
//! the walker hangs free of gravity and climbs with up and down.
//!
//! The ladder is level data, not a component: the tileset's custom value is
//! read by cell under the walker (`voltra_scene::tilemap::query`), so a
//! ladder is painted, not placed.

use voltra_core::Tick;
use voltra_ecs::{Entity, World};
use voltra_scene::tilemap::query::custom_under;
use voltra_scene::Transform;

/// World units per second the walker climbs at.
const CLIMB_SPEED: f32 = 3.0;

/// On the walker while it is on a ladder: it does not fall.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Climbing;

/// Whether the walker's middle is over a ladder tile.
pub fn on_ladder(world: &World, walker: Entity) -> bool {
    let Some(transform) = world.get::<Transform>(walker) else {
        return false;
    };
    custom_under(world, transform.translation, "ladder").is_some_and(|(_, _, value)| value.truthy())
}

/// Climbs the walker while it is on a ladder: no gravity, up and down at
/// [`CLIMB_SPEED`], held still with neither. Off one, it falls again. Call
/// after the walk has set the horizontal speed, and before the walker's
/// step moves it.
pub fn climb(tick: &mut Tick<'_>, walker: Entity) {
    let on = on_ladder(tick.world, walker);
    if !on {
        tick.world.remove::<Climbing>(walker);
        return;
    }
    // The keys say, jump or not: up is jump too, and a jump at full speed
    // would fling the walker off the top of the ladder.
    let direction = tick
        .input
        .action_axis("move_down", "move_up")
        .clamp(-1.0, 1.0);
    let mut velocity = crate::walker::velocity(tick.world, walker);
    velocity.y = direction * CLIMB_SPEED;
    tick.world
        .insert(walker, crate::walker::Motion { velocity });
    tick.world.insert(walker, Climbing);
}

#[cfg(test)]
mod tests {
    use voltra_render::glam::{IVec2, Vec2};
    use voltra_scene::tilemap::TileValue;
    use voltra_scene::{Tile, Tilemap};

    use super::*;

    #[test]
    fn the_walker_is_on_a_ladder_only_over_a_ladder_tile() {
        let mut world = World::new();
        let mut map = Tilemap::new(Vec2::splat(0.5));
        map.set(IVec2::new(0, 0), Some(Tile::new(3)));
        map.set(IVec2::new(1, 0), Some(Tile::new(0)));
        map.tile_data
            .entry(3)
            .or_default()
            .custom
            .insert("ladder".into(), TileValue::Bool(true));
        let layer = world.spawn();
        world.insert(layer, Transform::default());
        world.insert(layer, map);
        let walker = world.spawn();
        world.insert(walker, Transform::from_translation(Vec2::new(0.25, 0.25)));

        assert!(on_ladder(&world, walker));
        world
            .get_mut::<Transform>(walker)
            .expect("placed")
            .translation = Vec2::new(0.75, 0.25);
        assert!(!on_ladder(&world, walker));
    }
}
