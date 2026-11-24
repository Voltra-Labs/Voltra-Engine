//! The walker: found by its [`Walker`] mark, walked by the actions, and
//! told whether it has ground under its feet.

use voltra_core::{query, QueryFilter, Tick};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{RigidBody, Transform};

use crate::components::Walker;

/// World units per second the walker travels at full tilt.
const WALK_SPEED: f32 = 4.0;
/// Upward speed a jump starts with, in world units per second.
const JUMP_SPEED: f32 = 7.0;
/// Steepest surface still counted as ground, as the cosine of its angle.
///
/// `0.5` is 60°, Unity's default slope limit. Below it a wall would count as a
/// floor and the walker could climb it by holding the jump key.
const GROUND_NORMAL: f32 = 0.5;
/// How far past its feet the ground check looks.
///
/// Long enough to survive the fraction of a unit the solver leaves between two
/// resting bodies, short enough that a jump stops counting as grounded on the
/// frame it leaves the floor.
const GROUND_PROBE: f32 = 0.08;

/// The entity the keys move: the first carrying [`Walker`].
pub fn find(world: &World) -> Option<Entity> {
    world.query::<Walker>().map(|(entity, _)| entity).next()
}

/// Sets the walker's velocity from the move actions, and jumps it if
/// `jump` is asked for and it is standing on something.
///
/// Horizontal velocity is set rather than accelerated: this is a
/// character, not a crate, and a walker that slides to a halt is a physics
/// body pretending to be one.
pub fn steer(tick: &mut Tick<'_>, walker: Entity, jump: bool) {
    // Actions, from `assets/default.input.ron`: rebinding the keys is an
    // edit to that file, picked up while the game runs.
    let direction = tick.input.action_axis("move_left", "move_right");
    let jumped = jump && is_grounded(tick, walker);
    let Some(body) = tick.world.get_mut::<RigidBody>(walker) else {
        return;
    };
    body.velocity.x = direction.clamp(-1.0, 1.0) * WALK_SPEED;
    if jumped {
        body.velocity.y = JUMP_SPEED;
    }
}

/// Whether `entity` has ground under its feet.
///
/// A pair of short rays down from its feet, one at each side, rather than a
/// scan of the contacts: they answer before the first contact exists, and
/// one ray per foot keeps a walker whose middle overhangs the edge of the
/// ledge standing on it. Its size is read off its transform, so the editor
/// can make the walker any size. `QueryFilter` skips sensors by default, so
/// standing in a coin is not standing on one.
fn is_grounded(tick: &Tick<'_>, entity: Entity) -> bool {
    let Some(transform) = tick.world.get::<Transform>(entity) else {
        return false;
    };
    let half = (transform.scale * 0.5).abs();
    let filter = QueryFilter::new().excluding(entity);
    let reach = half.y + GROUND_PROBE;
    [-half.x, half.x].into_iter().any(|x| {
        let foot = transform.translation + Vec2::new(x, 0.0);
        query::ray(tick.world, foot, Vec2::NEG_Y, reach, filter)
            .is_some_and(|hit| hit.normal.y > GROUND_NORMAL)
    })
}

/// Puts the walker at `at`, standing still: after a load or a level change.
pub fn place(world: &mut World, walker: Entity, at: Vec2) {
    if let Some(transform) = world.get_mut::<Transform>(walker) {
        transform.translation = at;
    }
    if let Some(body) = world.get_mut::<RigidBody>(walker) {
        body.velocity = Vec2::ZERO;
    }
}
