//! A UI element that lays its children out in a row or a column.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::ui::stack::{Align, Direction};
use voltra_scene::{UiRect, UiStack};

use super::{active, ui_slice};

/// The stack section of a UI element, or the button that makes it one.
pub(super) fn show(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    let on_screen = world.get::<UiRect>(entity).is_some();
    let Some(stack) = world.get_mut::<UiStack>(entity) else {
        if on_screen && ui.button("Stack children").clicked() {
            world.insert(entity, UiStack::default());
            return Some("Add stack");
        }
        return None;
    };
    ui.separator();
    ui.label(RichText::new("Stack").strong())
        .on_hover_text("places the UI children one after another, by slot");
    let mut claim = None;
    egui::Grid::new("ui_stack").num_columns(2).show(ui, |ui| {
        ui.label("direction");
        ui.horizontal(|ui| {
            for (label, direction) in [("row", Direction::Row), ("column", Direction::Column)] {
                if ui
                    .selectable_value(&mut stack.direction, direction, label)
                    .changed()
                {
                    claim = Some("Edit stack");
                }
            }
        });
        ui.end_row();
        ui.label("spacing").on_hover_text("points between children");
        let drag = DragValue::new(&mut stack.spacing)
            .speed(0.5)
            .range(0.0..=f32::MAX);
        claim = claim.or(active(&ui.add(drag), "Edit stack"));
        ui.end_row();
        claim = claim.or(ui_slice::edges(
            ui,
            "padding",
            &mut stack.padding,
            "points kept clear inside",
        ));
        claim = claim.or(align(ui, "justify", &mut stack.justify, "along the stack"));
        claim = claim.or(align(ui, "align", &mut stack.align, "across the stack"));
    });
    if ui.small_button("Remove stack").clicked() {
        world.remove::<UiStack>(entity);
        claim = Some("Remove stack");
    }
    claim
}

fn align(ui: &mut Ui, label: &str, value: &mut Align, hint: &str) -> Option<&'static str> {
    ui.label(label).on_hover_text(hint);
    let mut claim = None;
    egui::ComboBox::from_id_salt(label)
        .selected_text(format!("{value:?}"))
        .show_ui(ui, |ui| {
            for option in [Align::Start, Align::Center, Align::End, Align::Stretch] {
                if ui
                    .selectable_value(value, option, format!("{option:?}"))
                    .changed()
                {
                    claim = Some("Edit stack");
                }
            }
        });
    ui.end_row();
    claim
}
