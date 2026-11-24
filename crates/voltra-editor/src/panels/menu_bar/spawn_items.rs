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
    if ui.button("Spawn HUD column").clicked() {
        spawn::record(editor, frame, "Spawn HUD column", |frame| {
            spawn::hud_column(frame, "HUD column")
        });
        ui.close();
    }
    if ui.button("Spawn HUD button").clicked() {
        spawn::record_all(editor, frame, "Spawn HUD button", |frame, _| {
            spawn::hud_button(frame, "HUD button")
        });
        ui.close();
    }
    if ui.button("Spawn HUD toggle").clicked() {
        spawn::record_all(editor, frame, "Spawn HUD toggle", |frame, _| {
            spawn::hud_toggle(frame, "HUD toggle")
        });
        ui.close();
    }
    if ui.button("Spawn HUD slider").clicked() {
        spawn::record_all(editor, frame, "Spawn HUD slider", |frame, _| {
            spawn::hud_slider(frame, "HUD slider")
        });
        ui.close();
    }
    if ui.button("Spawn HUD text field").clicked() {
        spawn::record(editor, frame, "Spawn HUD text field", |frame| {
            spawn::hud_text_field(frame, "HUD text field")
        });
        ui.close();
    }
    if ui.button("Spawn HUD scroll list").clicked() {
        spawn::record(editor, frame, "Spawn HUD scroll list", |frame| {
            spawn::hud_scroll_list(frame, "HUD scroll list")
        });
        ui.close();
    }
    if ui.button("Spawn particles").clicked() {
        spawn::record(editor, frame, "Spawn particles", |frame| {
            spawn::particles(frame, "Particles", Vec2::ZERO)
        });
        ui.close();
    }
    for (label, name, make) in [
        (
            "Spawn line",
            "Line",
            spawn::line as fn(&mut UiFrame<'_>, &str, Vec2) -> voltra_ecs::Entity,
        ),
        ("Spawn polygon", "Polygon", spawn::polygon),
        ("Spawn trail", "Trail", spawn::trail),
    ] {
        if ui.button(label).clicked() {
            spawn::record(editor, frame, label, |frame| make(frame, name, Vec2::ZERO));
            ui.close();
        }
    }
    if ui.button("Spawn skeleton").clicked() {
        spawn::record_all(editor, frame, "Spawn skeleton", |frame, _| {
            spawn::skeleton(frame, "Skeleton", Vec2::ZERO)
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
