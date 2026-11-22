//! Cameras that move themselves: following a target, and shaking.
//!
//! Godot's `Camera2D` does both itself — position smoothing, drag margins,
//! limits — and Unity hands them to Cinemachine; here they are two
//! components, [`CameraFollow2D`] and [`CameraShake`], on any entity, so a
//! light can follow the player the way a camera does. Neither is a field of
//! [`Camera`](crate::Camera): a camera that does not move is the common
//! authored case, and its file should not carry a rig.
//!
//! [`update`] runs once per frame, while the game runs: what is followed is
//! what is drawn, as the animations are.

pub mod follow;
pub mod shake;

pub use follow::{CameraFollow2D, FollowBounds, FollowState};
pub use shake::{CameraShake, ShakeState};

use voltra_ecs::World;
use voltra_render::glam::Vec2;

use crate::hierarchy::{parent_matrix, world_matrix};
use crate::Transform;

/// Moves every following entity towards its target, then shakes every
/// shaking one, by `delta` seconds; `aspect` is the frame's width over its
/// height, for a camera's view.
///
/// A shake's offset from the frame before is taken off first, so following
/// works from where the camera rests and a shake never drifts it.
pub fn update(world: &mut World, delta: f32, aspect: f32) {
    let delta = if delta.is_finite() {
        delta.max(0.0)
    } else {
        0.0
    };
    shake::settle(world);
    follow::update(world, delta, aspect);
    shake::update(world, delta);
}

/// Where `entity` is in the world.
fn world_position(world: &World, entity: voltra_ecs::Entity) -> Vec2 {
    world_matrix(world, entity).transform_point2(Vec2::ZERO)
}

/// Puts `entity` at `position` in the world, through its parent if it has
/// one.
fn set_world_position(world: &mut World, entity: voltra_ecs::Entity, position: Vec2) {
    let parent = parent_matrix(world, entity);
    let local = if parent.determinant().abs() > f32::EPSILON {
        parent.inverse().transform_point2(position)
    } else {
        position
    };
    if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.translation = local;
    }
}

#[cfg(test)]
mod tests;
