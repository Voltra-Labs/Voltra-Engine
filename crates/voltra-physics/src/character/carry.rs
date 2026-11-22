//! Moving platforms: a character carried by the floor it stood on.
//!
//! By the floor's own pose, not its velocity: a platform moved by a
//! kinematic body, by an animation clip or by the game's code moves its
//! riders the same way, and one that turns carries them round its centre.
//! Godot carries by the platform's velocity, which a clip or a script
//! setting the position does not have.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::{CharacterContact, Transform};

/// How far the floor `contact` names has carried a character at `at` since
/// it was stood on: moved, and turned about its own centre. Nothing for a
/// floor that is gone, or none.
pub(super) fn carry(world: &World, contact: &CharacterContact, at: Vec2) -> Vec2 {
    let (Some(floor), Some((was, was_turned))) = (contact.floor, contact.floor_pose) else {
        return Vec2::ZERO;
    };
    let Some(now) = world.get::<Transform>(floor) else {
        return Vec2::ZERO;
    };
    let turned = Vec2::from_angle(now.rotation - was_turned);
    let moved = now.translation + turned.rotate(at - was);
    let carried = moved - at;
    if carried.is_finite() {
        carried
    } else {
        Vec2::ZERO
    }
}
