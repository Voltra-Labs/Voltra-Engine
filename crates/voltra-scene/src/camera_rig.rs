//! Cameras that move themselves: following a target, and shaking.
//!
//! Godot's `Camera2D` does both itself — position smoothing, drag margins,
//! limits — and Unity hands them to Cinemachine; here they are two
//! components, [`CameraFollow2D`] on any entity, so a light can follow the
//! player the way a camera does, and [`CameraShake`] on a camera, whose
//! view it shakes. Neither is a field of
//! [`Camera2D`](crate::Camera2D): a camera that does not move is the common
//! authored case, and its file should not carry a rig.
//!
//! [`update`] runs once per frame, while the game runs: what is followed is
//! what is drawn, as the animations are.

pub mod follow;
pub mod shake;

pub use follow::{CameraFollow2D, FollowBounds, FollowState, FollowZoom};
pub use shake::{CameraShake, ShakeState};

use voltra_ecs::World;
use voltra_render::glam::Vec2;

use crate::hierarchy::{parent_matrix, world_matrix};
use crate::Transform2D;

/// Moves every following entity towards its target, then runs every
/// shake, by `delta` seconds. A camera's view is as wide as what it draws
/// into ([`camera::aspect`](crate::camera::aspect)): its viewport's share
/// of a screen of `screen_aspect` (width over height), or its render
/// texture's, found in `textures`.
///
/// A shake moves the view, not the transform, so following works from
/// where the camera rests and a shake never drifts it.
pub fn update(
    world: &mut World,
    delta: f32,
    screen_aspect: f32,
    textures: Option<&voltra_assets::Textures>,
) {
    let delta = if delta.is_finite() {
        delta.max(0.0)
    } else {
        0.0
    };
    follow::update(world, delta, screen_aspect, textures);
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
    if let Some(transform) = world.get_mut::<Transform2D>(entity) {
        transform.translation = local;
    }
}

#[cfg(test)]
mod group_tests;
#[cfg(test)]
mod shake_tests;
#[cfg(test)]
mod tests;
