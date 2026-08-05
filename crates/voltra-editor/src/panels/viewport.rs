//! Central panel: the rendered scene, and the pointer interaction on it.

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;

use crate::editor::Editor;

pub fn show(_editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::CentralPanel::default()
        // The scene brings its own background; the panel's would only show
        // as a border around the image.
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            let available = ui.available_size();
            // egui lays out in logical points while the target is sized in
            // physical pixels. Missing this conversion renders the scene at
            // half resolution on a 200% display.
            let scale = ui.ctx().pixels_per_point();
            frame.request_viewport_size((available.x * scale) as u32, (available.y * scale) as u32);

            let scene = ui.add(
                egui::Image::new(egui::load::SizedTexture::new(frame.viewport(), available))
                    // Without this the image is inert decoration and the
                    // pointer never reaches the camera.
                    .sense(egui::Sense::drag()),
            );

            if scene.dragged_by(egui::PointerButton::Middle) {
                pan(frame, scene.drag_delta(), available.y);
            }
        });
}

/// Drags the scene under the pointer by `delta`, in egui points.
fn pan(frame: &mut UiFrame<'_>, delta: egui::Vec2, height_in_points: f32) {
    // One scalar rather than one per axis: the camera's aspect *is* the panel's
    // aspect, so world units per point come out the same either way. If they
    // ever differed, the image would already be stretched.
    let world_per_point = 2.0 * frame.camera.half_extents().y / height_in_points.max(1.0);

    // Grab-and-drag, so the camera travels opposite the pointer. Y is negated
    // on top of that because egui counts points downwards and the world counts
    // them up.
    frame.camera.position.x -= delta.x * world_per_point;
    frame.camera.position.y += delta.y * world_per_point;
}
