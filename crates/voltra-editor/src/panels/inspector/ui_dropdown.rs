//! A dropdown: its options, the chosen one, and how its list looks.

use voltra_core::egui::{self, RichText, TextEdit, Ui};
use voltra_scene::UiDropdown;

use super::active;

pub(super) fn show(ui: &mut Ui, dropdown: &mut UiDropdown) -> Option<&'static str> {
    ui.separator();
    ui.label(RichText::new("Dropdown").strong())
        .on_hover_text("a click opens the list over everything; its Text is the style");
    let mut claim = None;

    // One option per line: what a list of words is easiest to edit as.
    let mut lines = dropdown.options.join("\n");
    let response = ui.add(
        TextEdit::multiline(&mut lines)
            .desired_rows(3)
            .desired_width(f32::INFINITY)
            .hint_text("one option per line"),
    );
    if response.changed() {
        dropdown.options = lines.lines().map(str::to_owned).collect();
    }
    claim = claim.or(active(&response, "Edit dropdown options"));

    egui::Grid::new("ui_dropdown")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("chosen");
            let last = dropdown.options.len().saturating_sub(1);
            claim = claim.or(active(
                &ui.add(egui::DragValue::new(&mut dropdown.selected).range(0..=last)),
                "Choose dropdown option",
            ));
            ui.end_row();
            ui.label("items shown")
                .on_hover_text("the most the list shows at once; the highlight scrolls it");
            claim = claim.or(active(
                &ui.add(egui::DragValue::new(&mut dropdown.max_visible).range(1..=64)),
                "Edit dropdown",
            ));
            ui.end_row();
            for (label, color) in [
                ("list", &mut dropdown.list_color),
                ("highlight", &mut dropdown.highlight_color),
            ] {
                ui.label(label);
                let popup = ui.auto_id_with(label);
                let response = ui.color_edit_button_rgba_unmultiplied(color);
                if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
                    claim = claim.or(Some("Edit dropdown"));
                }
                ui.end_row();
            }
        });
    claim
}
