//! A button's colours by state, the atlas frames it shows in each, and
//! whether it takes input at all.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::ui::ButtonFrames;
use voltra_scene::UiButton;

use super::active;

/// `frame` is the frame its sprite shows now: where state frames start
/// when they are turned on.
pub(super) fn show(ui: &mut Ui, button: &mut UiButton, frame: u32) -> Option<&'static str> {
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
            let response = crate::panels::colour_edit::rgba(ui, colour);
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
        ui.label("state frames").on_hover_text(
            "swap the sprite's atlas frame by state: four in a row from the current one",
        );
        let mut framed = button.frames.is_some();
        if ui.checkbox(&mut framed, "").changed() {
            button.frames = framed.then(|| ButtonFrames::in_a_row(frame));
            claim = claim.or(Some("Toggle button frames"));
        }
        ui.end_row();
        if let Some(frames) = &mut button.frames {
            for (label, value) in [
                ("  normal", &mut frames.normal),
                ("  hovered", &mut frames.hovered),
                ("  pressed", &mut frames.pressed),
                ("  disabled", &mut frames.disabled),
            ] {
                ui.label(label);
                claim = claim.or(active(
                    &ui.add(DragValue::new(value).speed(0.1)),
                    "Edit button frames",
                ));
                ui.end_row();
            }
        }
    });
    claim
}
