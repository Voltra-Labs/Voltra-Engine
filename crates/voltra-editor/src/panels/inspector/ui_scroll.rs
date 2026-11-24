//! A UI element that scrolls its children and masks them, or the button
//! that makes it one.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::{UiRect, UiScroll};

use super::active;

pub(super) fn show(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    let on_screen = world.get::<UiRect>(entity).is_some();
    let Some(scroll) = world.get_mut::<UiScroll>(entity) else {
        if on_screen && ui.button("Scroll children").clicked() {
            world.insert(entity, UiScroll::default());
            return Some("Add scroll");
        }
        return None;
    };
    ui.separator();
    ui.label(RichText::new("Scroll").strong())
        .on_hover_text("moves its UI children by the offset and cuts them to its rect");
    let mut claim = None;
    egui::Grid::new("ui_scroll").num_columns(2).show(ui, |ui| {
        ui.label("axes");
        ui.horizontal(|ui| {
            if ui.checkbox(&mut scroll.vertical, "vertical").changed()
                | ui.checkbox(&mut scroll.horizontal, "horizontal").changed()
            {
                claim = Some("Edit scroll");
            }
        });
        ui.end_row();
        ui.label("offset");
        ui.horizontal(|ui| {
            for value in [&mut scroll.offset.x, &mut scroll.offset.y] {
                let drag = DragValue::new(value).speed(1.0).range(0.0..=f32::MAX);
                claim = claim.or(active(&ui.add(drag), "Edit scroll"));
            }
        });
        ui.end_row();
        ui.label("step").on_hover_text("points per wheel line");
        let drag = DragValue::new(&mut scroll.step)
            .speed(1.0)
            .range(1.0..=f32::MAX);
        claim = claim.or(active(&ui.add(drag), "Edit scroll"));
        ui.end_row();
    });
    if ui.small_button("Remove scroll").clicked() {
        world.remove::<UiScroll>(entity);
        claim = Some("Remove scroll");
    }
    claim
}
