//! Turning a click on the scene image into a selection.
//!
//! The conversion is the reason `Camera2D::viewport_to_world` exists: egui
//! reports a pointer in screen points, the world is in world units, and the
//! camera is the only thing that knows the mapping between them.

use voltra_core::egui::Response;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::pick;

/// The entity under the pointer for the interaction `response` describes.
///
/// Returns `None` both when the click landed on empty space and when there was
/// no interaction at all, so callers must test `Response::clicked` first rather
/// than clearing the selection on every frame.
pub fn clicked_entity(response: &Response, frame: &UiFrame<'_>) -> Option<Entity> {
    let pointer = response.interact_pointer_pos()?;

    // `interact_pointer_pos` is in global screen points; the camera works in
    // viewport-local ones, so the panel's own corner comes off first.
    let local = pointer - response.rect.min;
    let viewport = Vec2::new(response.rect.width(), response.rect.height());
    let world = frame
        .camera
        .viewport_to_world(Vec2::new(local.x, local.y), viewport);

    pick::sprite_at(frame.world, world)
}
