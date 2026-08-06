//! Central panel: the rendered scene image.
//!
//! Layout only. What the pointer and keyboard do with it is
//! [`crate::camera::ViewportCamera`]'s job.

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;

use crate::editor::Editor;

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
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
                    // pointer never reaches the camera or the selection.
                    .sense(egui::Sense::click_and_drag()),
            );

            // Before navigation: a click and a drag are mutually exclusive in
            // egui, so this cannot swallow a pan.
            if scene.clicked() {
                editor.selected = crate::picking::clicked_entity(&scene, frame);
            }

            editor.camera.navigate(ui, &scene, frame.camera);
        });
}
