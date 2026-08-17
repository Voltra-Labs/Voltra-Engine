//! Central panel: the rendered scene image.
//!
//! Layout only. What the pointer and keyboard do with it is
//! [`crate::camera::ViewportCamera`]'s job.

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;

use crate::editor::Editor;
use crate::play::PlayState;
use crate::tool::Tool;

/// What the viewport image is multiplied by while the editor is not editing.
///
/// Unity's play-mode tint, and for Unity's reason: the oldest complaint about
/// that editor is work lost because play mode was not obvious. Applied through
/// egui's image tint, so it is a UI-level effect — nothing in the render path
/// or in the scene knows that play mode exists.
const PLAY_TINT: egui::Color32 = egui::Color32::from_rgb(150, 175, 225);

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

            let mut image =
                egui::Image::new(egui::load::SizedTexture::new(frame.viewport(), available))
                    // Without this the image is inert decoration and the
                    // pointer never reaches the camera or the selection.
                    .sense(egui::Sense::click_and_drag());
            if editor.play.state() != PlayState::Editing {
                image = image.tint(PLAY_TINT);
            }
            let scene = ui.add(image);

            // `W` picks the translate tool. Scoped the same way the camera
            // scopes its own keys — hovered, and egui not wanting the keyboard
            // — so typing a `w` into the inspector cannot switch tools.
            if scene.hovered() && !ui.ctx().egui_wants_keyboard_input() {
                ui.input(|i| {
                    if i.key_pressed(egui::Key::W) {
                        editor.tool = Tool::Translate;
                    }
                });
            }

            // The gizmo gets first refusal, for the same reason egui gets it
            // before the scene does: a handle drawn over a sprite has to be
            // grabbable, and a drag on one must not also re-select.
            let outcome = match editor.tool {
                Tool::Translate => editor.gizmo.update(&scene, frame, editor.selected),
            };

            // Every frame the drag holds an entity, so the history keeps one
            // entry open for the whole drag instead of one per frame. The entry
            // closes on the release frame, which is the first frame no claim
            // arrives.
            if outcome.dragging.is_some() {
                editor.history.claim("Move");
            }

            // Before navigation: a click and a drag are mutually exclusive in
            // egui, so this cannot swallow a pan.
            if !outcome.consumed && scene.clicked() {
                editor.selected = crate::picking::clicked_entity(&scene, frame);
            }

            editor.camera.navigate(ui, &scene, frame.camera);

            // After navigation, so the arms are laid out against the camera
            // this frame ends with rather than the one it started with.
            match editor.tool {
                Tool::Translate => editor.gizmo.draw(&scene, frame, editor.selected),
            }

            // Last of all: the overlay is drawn in the order it is pushed, and
            // a collider outline must not cover the handle being dragged over
            // it.
            if editor.show_colliders {
                let contacts = frame.contacts();
                let (world, lines) = frame.world_and_lines();
                voltra_physics::debug::draw(world, contacts, lines);
            }
        });
}
