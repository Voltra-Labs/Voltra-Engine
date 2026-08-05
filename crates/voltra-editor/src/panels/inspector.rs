//! Right panel: the components of the selected entity.

use voltra_core::egui::{self, Color32, DragValue, RichText, Ui};
use voltra_core::UiFrame;
use voltra_scene::{Sprite, Transform};

use crate::editor::Editor;

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::Panel::right("inspector")
        .default_size(240.0)
        .show(ui, |ui| {
            ui.heading("Inspector");
            ui.separator();

            // A stale handle is normal, not a bug: the entity may have been
            // despawned since it was selected.
            let Some(entity) = editor.selected.filter(|e| frame.world.is_alive(*e)) else {
                ui.label(RichText::new("nothing selected").italics().weak());
                return;
            };

            ui.label(format!(
                "Entity {} (gen {})",
                entity.index(),
                entity.generation()
            ));
            ui.separator();

            if let Some(transform) = frame.world.get_mut::<Transform>(entity) {
                transform_ui(ui, transform);
            }
            if let Some(sprite) = frame.world.get_mut::<Sprite>(entity) {
                ui.separator();
                sprite_ui(ui, sprite);
            }

            ui.separator();
            if ui
                .button(RichText::new("Delete").color(Color32::LIGHT_RED))
                .clicked()
            {
                frame.world.despawn(entity);
                editor.selected = None;
            }
        });
}

fn transform_ui(ui: &mut Ui, transform: &mut Transform) {
    ui.label(RichText::new("Transform").strong());

    egui::Grid::new("transform").num_columns(2).show(ui, |ui| {
        ui.label("position");
        ui.horizontal(|ui| {
            ui.add(DragValue::new(&mut transform.translation.x).speed(0.01));
            ui.add(DragValue::new(&mut transform.translation.y).speed(0.01));
        });
        ui.end_row();

        ui.label("rotation");
        ui.drag_angle(&mut transform.rotation);
        ui.end_row();

        ui.label("scale");
        ui.horizontal(|ui| {
            ui.add(DragValue::new(&mut transform.scale.x).speed(0.01));
            ui.add(DragValue::new(&mut transform.scale.y).speed(0.01));
        });
        ui.end_row();
    });
}

fn sprite_ui(ui: &mut Ui, sprite: &mut Sprite) {
    ui.label(RichText::new("Sprite").strong());
    ui.horizontal(|ui| {
        ui.label("colour");
        ui.color_edit_button_rgba_unmultiplied(&mut sprite.color);
    });
}
