//! A text field's text, placeholder and limit.

use voltra_core::egui::{self, RichText, TextEdit, Ui};
use voltra_scene::UiTextField;

pub(super) fn show(ui: &mut Ui, field: &mut UiTextField) -> Option<&'static str> {
    ui.separator();
    ui.label(RichText::new("Text field").strong())
        .on_hover_text("typed into when focused; its Text is the style");
    let mut claim = None;
    egui::Grid::new("ui_text_field")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("text");
            let response = ui.add(TextEdit::singleline(&mut field.text));
            if response.has_focus() || response.changed() {
                claim = Some("Edit text field");
            }
            ui.end_row();
            ui.label("placeholder");
            let response = ui.add(TextEdit::singleline(&mut field.placeholder));
            if response.has_focus() || response.changed() {
                claim = claim.or(Some("Edit text field"));
            }
            ui.end_row();
            ui.label("limit")
                .on_hover_text("most characters; off for none");
            let mut limited = field.max_chars.is_some();
            ui.horizontal(|ui| {
                if ui.checkbox(&mut limited, "").changed() {
                    field.max_chars = limited.then_some(field.text.chars().count().max(16));
                    claim = claim.or(Some("Edit text field"));
                }
                if let Some(max) = &mut field.max_chars {
                    let response = ui.add(egui::DragValue::new(max).range(1..=10_000));
                    if response.dragged() || response.has_focus() {
                        claim = claim.or(Some("Edit text field"));
                    }
                }
            });
            ui.end_row();
            ui.label("selection");
            let popup = ui.auto_id_with("popup");
            let response = ui.color_edit_button_rgba_unmultiplied(&mut field.selection_color);
            if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
                claim = claim.or(Some("Edit text field"));
            }
            ui.end_row();
        });
    claim
}
