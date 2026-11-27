//! The row prefab mode adds to the toolbar: the trail from the scene to
//! the prefab being edited, the way back, Save, and a variant of it.
//!
//! Unity's Prefab Mode breadcrumb, with its back arrow. A row on screen
//! rather than the window title alone, because the one thing an author
//! must not miss is that the world on screen is not their level.

use voltra_core::egui::{RichText, Ui};
use voltra_core::UiFrame;

use crate::editor::Editor;
use crate::scene_dialogs;

/// The row, in prefab mode; nothing otherwise.
pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    let Some(back_to) = editor.prefab_mode.returns_to().map(str::to_owned) else {
        return;
    };
    ui.separator();
    ui.horizontal(|ui| {
        if ui
            .button("◀ Back")
            .on_hover_text(format!(
                "return to {back_to}; its instances follow the prefab as saved"
            ))
            .clicked()
        {
            editor.prefab_mode.request_back();
        }
        for name in editor.prefab_mode.trail() {
            ui.label(RichText::new(name).weak());
            ui.label(RichText::new("›").weak());
        }
        let star = if editor.document.is_dirty(&editor.history) {
            "*"
        } else {
            ""
        };
        ui.label(RichText::new(format!("{}{star}", editor.document.name())).strong());
        ui.separator();
        if ui
            .button("Save prefab")
            .on_hover_text("write the prefab's file (Ctrl+S)")
            .clicked()
        {
            scene_dialogs::save(editor, frame);
        }
        if let Some(path) = editor.document.path().cloned() {
            if ui
                .button("Create variant")
                .on_hover_text("a new prefab following this one as saved, beside it")
                .clicked()
            {
                if let Err(e) = crate::prefab::create_variant(editor, frame, &path) {
                    log::error!("{e}");
                }
            }
        }
        ui.label(RichText::new("editing a prefab: its instances follow what is saved").weak());
    });
}
