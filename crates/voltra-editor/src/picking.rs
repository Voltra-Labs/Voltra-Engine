//! Turning a click on the scene image into a selection.
//!
//! The conversion is the reason `Camera2D::viewport_to_world` exists: egui
//! reports a pointer in screen points, the world is in world units, and the
//! camera is the only thing that knows the mapping between them.

use voltra_core::egui::{Pos2, Response};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{pick, ui, UiLayout};

/// The HUD element, sprite, text or tilemap layer under the pointer for the
/// interaction `response` describes.
///
/// Returns `None` both when the click landed on empty space and when there was
/// no interaction at all, so callers must test `Response::clicked` first rather
/// than clearing the selection on every frame.
pub fn clicked_entity(response: &Response, frame: &UiFrame<'_>) -> Option<Entity> {
    let pointer = response.interact_pointer_pos()?;
    // The HUD first: it is drawn over the whole world, so whatever of it is
    // under the pointer is what the click visibly landed on. In points from
    // the viewport's corner, which is what the HUD is laid out in.
    let local = pointer - response.rect.min;
    let screen = Vec2::new(response.rect.width(), response.rect.height());
    let layout = UiLayout::from_world(frame.world, screen);
    if let Some(element) = ui::element_at(frame.world, &layout, Vec2::new(local.x, local.y)) {
        return Some(element);
    }
    // Then a bone, drawn over the sprites hung on it: within six pixels of
    // its line.
    let at = world_at(response, pointer, frame);
    let per_pixel = frame.camera.half_extents().x * 2.0 / response.rect.width().max(1.0);
    if let Some(bone) = voltra_scene::skeleton::bone_at(frame.world, at, 6.0 * per_pixel) {
        return Some(bone);
    }
    pick::entity_at(
        frame.world,
        world_at(response, pointer, frame),
        frame.sheets(),
    )
}

/// Where a screen point over `response` is in the world.
///
/// Separate from the pick because a drop needs the position and not what is
/// under it: an asset dragged onto the viewport lands where it was released,
/// whether or not a sprite is already there.
pub fn world_at(response: &Response, pointer: Pos2, frame: &UiFrame<'_>) -> Vec2 {
    // Screen points are global; the camera works in viewport-local ones, so the
    // panel's own corner comes off first.
    let local = pointer - response.rect.min;
    let viewport = Vec2::new(response.rect.width(), response.rect.height());
    frame
        .camera
        .viewport_to_world(Vec2::new(local.x, local.y), viewport)
}
