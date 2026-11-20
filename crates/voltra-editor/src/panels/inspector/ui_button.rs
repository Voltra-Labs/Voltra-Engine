//! A button's colours by state, and whether it takes input at all.

use voltra_core::egui::{self, RichText, Ui};
use voltra_scene::UiButton;

pub(super) fn show(ui: &mut Ui, button: &mut UiButton) -> Option<&'static str> {
    ui.label(RichText::new("Button").strong())
        .on_hover_text("clickable and focusable; tints this entity's sprite by state");
    let mut claim = None;
    egui::Grid::new("ui_button").num_columns(2).show(ui, |ui| {
        for (label, colour) in [
            ("normal", &mut button.normal),
            ("hovered", &mut button.hovered),
            ("pressed", &mut button.pressed),
            ("disabled", &mut button.disabled_color),
        ] {
            ui.label(label);
            // A popup, as every colour field is: see `sprite::show`.
            let popup = ui.auto_id_with("popup");
            let response = ui.color_edit_button_rgba_unmultiplied(colour);
            if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
                claim = claim.or(Some("Edit button"));
            }
            ui.end_row();
        }
        ui.label("disabled");
        if ui.checkbox(&mut button.disabled, "").changed() {
            claim = claim.or(Some("Toggle button"));
        }
        ui.end_row();
    });
    claim
}
