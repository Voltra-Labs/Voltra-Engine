//! Where a camera's view is: its entity's place and turn, the shake on top,
//! and the shape of what it draws into.

use std::f32::consts::{PI, TAU};

use voltra_assets::Textures;
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::{Camera2D, CameraTarget};
use crate::camera_rig::CameraShake;
use crate::hierarchy::world_matrix;
use crate::transform::Transform2D;

/// Turns closer to upright than this are upright: a parent's `0.3` under a
/// child's `-0.3` leaves a rounding error, not a turn, and a turned view is
/// never pixel-snapped.
pub const UPRIGHT: f32 = 1e-6;

/// `turn` brought into `-π..=π`, and to exactly `0` within [`UPRIGHT`] of
/// it; a turn that is not a number is upright. A whole turn is upright.
pub fn normalised_turn(turn: f32) -> f32 {
    if !turn.is_finite() {
        return 0.0;
    }
    let mut turn = turn.rem_euclid(TAU);
    if turn > PI {
        turn -= TAU;
    }
    if turn.abs() < UPRIGHT || (TAU - turn.abs()) < UPRIGHT {
        0.0
    } else {
        turn
    }
}

/// Where `entity` rests in the world and how far it is turned, in radians —
/// what a follower moves and fits by, without any shake.
///
/// The position is the entity's, through its parents. The turn is its own
/// [`Transform2D::rotation`], and its parents' too only when its [`Camera2D`]
/// says [`Camera2D::inherit_rotation`] (an entity without a camera turns with
/// its parents): a camera on a rolling ball stays upright unless asked.
/// Normalised by [`normalised_turn`].
pub fn placement(world: &World, entity: Entity) -> (Vec2, f32) {
    let matrix = world_matrix(world, entity);
    let position = matrix.transform_point2(Vec2::ZERO);
    let inherit = world
        .get::<Camera2D>(entity)
        .is_none_or(|camera| camera.inherit_rotation);
    let rotation = if inherit {
        Transform2D::from_matrix(matrix).rotation
    } else {
        world
            .get::<Transform2D>(entity)
            .map_or(0.0, |transform| transform.rotation)
    };
    (position, normalised_turn(rotation))
}

/// Where the view through `entity` is this frame: its [`placement`] with
/// its [`CameraShake`] on top — the shake's offset along the view's own
/// axes, its angle added to the turn. The shake moves the view, never the
/// entity, so nothing that writes the transform can make it drift.
pub fn eye(world: &World, entity: Entity) -> (Vec2, f32) {
    let (position, rotation) = placement(world, entity);
    let Some(shake) = world.get::<CameraShake>(entity) else {
        return (position, rotation);
    };
    let offset = Vec2::from_angle(rotation).rotate(shake.applied_offset());
    let shaken = position + offset;
    (
        if shaken.is_finite() { shaken } else { position },
        normalised_turn(rotation + shake.applied_angle()),
    )
}

/// The width over the height of what `entity`'s camera draws into, on a
/// screen of `screen` aspect: its render texture's, when it has one
/// resolved in `textures`, or its viewport's share of the screen — what
/// [`Shot::view`](super::Shot::view) draws it at. The screen's aspect for
/// an entity without a camera.
pub fn aspect(world: &World, entity: Entity, screen: f32, textures: Option<&Textures>) -> f32 {
    let screen = if screen.is_finite() && screen > 0.0 {
        screen
    } else {
        1.0
    };
    let Some(camera) = world.get::<Camera2D>(entity) else {
        return screen;
    };
    let texture = world
        .get::<CameraTarget>(entity)
        .and_then(|target| target.handle)
        .zip(textures)
        .and_then(|(handle, textures)| textures.render_texture(handle));
    if let Some(texture) = texture {
        let (w, h) = (texture.picture.width(), texture.picture.height());
        return w.max(1) as f32 / h.max(1) as f32;
    }
    let viewport = camera.viewport.clamped();
    if viewport.width > 0.0 && viewport.height > 0.0 {
        screen * viewport.width / viewport.height
    } else {
        screen
    }
}
