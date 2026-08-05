//! Left panel: every entity in the scene, and which one is selected.

use voltra_core::egui::{self, RichText, Ui};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_scene::Sprite;

use crate::editor::Editor;

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::Panel::left("hierarchy")
        .default_size(200.0)
        .show(ui, |ui| {
            ui.heading("Hierarchy");
            ui.separator();

            // Collected before the loop: selecting inside it would hold a
            // borrow of the world while the buttons want to mutate it.
            let mut entities: Vec<Entity> = frame.world.query::<Sprite>().map(|(e, _)| e).collect();
            // Storage order is not list order. A sparse set fills the hole
            // left by a removal with its last element, so deleting one row
            // would otherwise make another jump across the list.
            entities.sort_by_key(Entity::index);

            if entities.is_empty() {
                ui.label(RichText::new("nothing in the scene").italics().weak());
                return;
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                for entity in entities {
                    let label = format!("Entity {}", entity.index());
                    if ui
                        .selectable_label(editor.selected == Some(entity), label)
                        .clicked()
                    {
                        editor.selected = Some(entity);
                    }
                }
            });
        });
}
