//! The selection's Delete button.

use voltra_core::egui::{Color32, RichText, Ui};
use voltra_core::UiFrame;

use crate::editor::Editor;

/// Deletes the selection, and everything under it, when the button is
/// clicked. Says how many when it is more than one.
pub(super) fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    let label = match editor.selection.len() {
        0 | 1 => "Delete".to_owned(),
        n => format!("Delete {n}"),
    };
    if ui
        .button(RichText::new(label).color(Color32::LIGHT_RED))
        .on_hover_text("Delete")
        .clicked()
    {
        crate::delete::selection(editor, frame);
    }
}
