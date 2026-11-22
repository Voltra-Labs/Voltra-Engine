//! The Scene menu's spawn buttons: one per kind of entity, each one undo
//! entry.

use voltra_core::egui::Ui;
use voltra_core::UiFrame;
use voltra_render::glam::Vec2;

use crate::editor::Editor;
use crate::spawn;

pub(super) fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    if ui.button("Spawn sprite").clicked() {
        spawn::record(editor, frame, "Spawn sprite", |frame| {
            spawn::sprite(frame, "Sprite", Vec2::ZERO)
        });
        ui.close();
    }
    if ui.button("Spawn tilemap").clicked() {
        spawn::record(editor, frame, "Spawn tilemap", |frame| {
            spawn::tilemap(frame, "Tilemap", Vec2::ZERO)
        });
        ui.close();
    }
    if ui.button("Spawn text").clicked() {
        spawn::record(editor, frame, "Spawn text", |frame| {
            spawn::text(frame, "Text", Vec2::ZERO, None)
        });
        ui.close();
    }
    if ui.button("Spawn HUD text").clicked() {
        spawn::record(editor, frame, "Spawn HUD text", |frame| {
            spawn::hud_text(frame, "HUD text")
        });
        ui.close();
    }
    if ui.button("Spawn HUD panel").clicked() {
        spawn::record(editor, frame, "Spawn HUD panel", |frame| {
            spawn::hud_panel(frame, "HUD panel")
        });
        ui.close();
    }
    if ui.button("Spawn HUD button").clicked() {
        spawn::record_all(editor, frame, "Spawn HUD button", |frame, _| {
            spawn::hud_button(frame, "HUD button")
        });
        ui.close();
    }
    if ui.button("Spawn particles").clicked() {
        spawn::record(editor, frame, "Spawn particles", |frame| {
            spawn::particles(frame, "Particles", Vec2::ZERO)
        });
        ui.close();
    }
    if ui.button("Spawn light").clicked() {
        spawn::record(editor, frame, "Spawn light", |frame| {
            spawn::light(frame, "Light", Vec2::ZERO)
        });
        ui.close();
    }
    if ui.button("Spawn global light").clicked() {
        spawn::record(editor, frame, "Spawn global light", |frame| {
            spawn::global_light(frame, "Global light")
        });
        ui.close();
    }
    if ui.button("Spawn camera").clicked() {
        spawn::record(editor, frame, "Spawn camera", |frame| {
            spawn::camera(frame, "Camera", Vec2::ZERO)
        });
        ui.close();
    }
}
