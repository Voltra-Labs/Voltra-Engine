//! A tooltip: what it says, when it shows, how it looks.

use voltra_core::egui::{self, DragValue, RichText, TextEdit, Ui};
use voltra_scene::UiTooltip;

use super::active;

pub(super) fn show(ui: &mut Ui, tooltip: &mut UiTooltip) -> Option<&'static str> {
    ui.separator();
    ui.label(RichText::new("Tooltip").strong())
        .on_hover_text("shown after a hover, or while the element has the focus");
    let mut claim = active(
        &ui.add(
            TextEdit::multiline(&mut tooltip.text)
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        ),
        "Edit tooltip",
    );
    egui::Grid::new("ui_tooltip").num_columns(2).show(ui, |ui| {
        ui.label("delay").on_hover_text("seconds before it shows");
        claim = claim.or(active(
            &ui.add(
                DragValue::new(&mut tooltip.delay)
                    .speed(0.05)
                    .range(0.0..=10.0),
            ),
            "Edit tooltip",
        ));
        ui.end_row();
        ui.label("size")
            .on_hover_text("its text's em height, in points");
        claim = claim.or(active(
            &ui.add(
                DragValue::new(&mut tooltip.size)
                    .speed(0.1)
                    .range(1.0..=200.0),
            ),
            "Edit tooltip",
        ));
        ui.end_row();
        ui.label("wrap")
            .on_hover_text("the widest its text runs, in points");
        claim = claim.or(active(
            &ui.add(DragValue::new(&mut tooltip.max_width).range(10.0..=4000.0)),
            "Edit tooltip",
        ));
        ui.end_row();
        for (label, color) in [
            ("background", &mut tooltip.background),
            ("text colour", &mut tooltip.color),
        ] {
            ui.label(label);
            let popup = ui.auto_id_with(label);
            let response = ui.color_edit_button_rgba_unmultiplied(color);
            if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
                claim = claim.or(Some("Edit tooltip"));
            }
            ui.end_row();
        }
    });
    claim
}
