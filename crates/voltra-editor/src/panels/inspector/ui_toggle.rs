//! A toggle: whether it is on, and how on looks.

use voltra_core::egui::{self, RichText, Ui};
use voltra_scene::UiToggle;

pub(super) fn show(ui: &mut Ui, toggle: &mut UiToggle) -> Option<&'static str> {
    ui.separator();
    ui.label(RichText::new("Toggle").strong())
        .on_hover_text("each click of the button flips it");
    let mut claim = None;
    egui::Grid::new("ui_toggle").num_columns(2).show(ui, |ui| {
        ui.label("on");
        if ui.checkbox(&mut toggle.on, "").changed() {
            claim = Some("Toggle toggle");
        }
        ui.end_row();
        ui.label("on tint")
            .on_hover_text("multiplied into the button's colour while on");
        let popup = ui.auto_id_with("popup");
        let response = crate::panels::colour_edit::rgba(ui, &mut toggle.on_tint);
        if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
            claim = claim.or(Some("Edit toggle"));
        }
        ui.end_row();
    });
    claim
}
