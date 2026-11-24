//! A slider's range and value, and which part of one a child is.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::ui::stack::Direction;
use voltra_scene::{UiSlider, UiSliderPart};

use super::active;

pub(super) fn show(ui: &mut Ui, slider: &mut UiSlider) -> Option<&'static str> {
    ui.separator();
    ui.label(RichText::new("Slider").strong())
        .on_hover_text("dragged along its rect, or stepped by the navigation when focused");
    let mut claim = None;
    egui::Grid::new("ui_slider").num_columns(2).show(ui, |ui| {
        for (label, value) in [
            ("value", &mut slider.value),
            ("min", &mut slider.min),
            ("max", &mut slider.max),
        ] {
            ui.label(label);
            claim = claim.or(active(
                &ui.add(DragValue::new(value).speed(0.05)),
                "Edit slider",
            ));
            ui.end_row();
        }
        ui.label("step")
            .on_hover_text("what the value snaps to; 0 for none");
        let drag = DragValue::new(&mut slider.step)
            .speed(0.05)
            .range(0.0..=f32::MAX);
        claim = claim.or(active(&ui.add(drag), "Edit slider"));
        ui.end_row();
        ui.label("direction");
        ui.horizontal(|ui| {
            for (label, direction) in [("row", Direction::Row), ("column", Direction::Column)] {
                if ui
                    .selectable_value(&mut slider.direction, direction, label)
                    .changed()
                {
                    claim = Some("Edit slider");
                }
            }
        });
        ui.end_row();
    });
    claim
}

pub(super) fn show_part(ui: &mut Ui, part: &mut UiSliderPart) -> Option<&'static str> {
    let mut claim = None;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Slider part").strong())
            .on_hover_text("moved by the slider it is a child of");
        for (label, kind) in [
            ("fill", UiSliderPart::Fill),
            ("handle", UiSliderPart::Handle),
        ] {
            if ui.selectable_value(part, kind, label).changed() {
                claim = Some("Edit slider part");
            }
        }
    });
    claim
}
