//! The timeline's second row: Record, the clip's own Undo and Redo, and the
//! keys' Copy and Paste.

use voltra_assets::AssetPath;
use voltra_core::egui::{self, RichText, Ui};
use voltra_scene::AnimationClips;

use crate::timeline::Timeline;
use crate::undo::shortcut::UndoAction;

pub(super) fn show(
    ui: &mut Ui,
    timeline: &mut Timeline,
    clips: &mut AnimationClips,
    path: &AssetPath,
) {
    ui.horizontal(|ui| {
        let colour = if timeline.record {
            ui.visuals().error_fg_color
        } else {
            ui.visuals().text_color()
        };
        let record = ui
            .toggle_value(&mut timeline.record, RichText::new("Record").color(colour))
            .on_hover_text(
                "moving or editing an entity under this player keys it at the playhead, instead of changing the scene",
            );
        if record.changed() && timeline.record {
            // Recording keys what the preview shows.
            timeline.preview = true;
        }
        if !timeline.preview {
            timeline.record = false;
        }
        ui.separator();
        let handle = clips.load(path);
        let (can_undo, can_redo) = timeline.can_undo_redo(path, clips.get(handle));
        if ui
            .add_enabled(can_undo, egui::Button::new("Undo"))
            .on_hover_text("the clip's own history: Ctrl+Z over the timeline, or anywhere while recording")
            .clicked()
        {
            timeline.undo_redo(UndoAction::Undo, clips);
        }
        if ui.add_enabled(can_redo, egui::Button::new("Redo")).clicked() {
            timeline.undo_redo(UndoAction::Redo, clips);
        }
        ui.separator();
        if ui
            .button("Copy")
            .on_hover_text("the selected key, or every key at the playhead (Ctrl+C over the timeline)")
            .clicked()
        {
            timeline.copy_keys(clips);
        }
        let paste = ui
            .add_enabled(!timeline.copied.is_empty(), egui::Button::new("Paste"))
            .on_hover_text(
                "at the playhead: one key into the selected track if it takes it, else each into its own (Ctrl+V)",
            );
        if paste.clicked() {
            timeline.paste_keys(clips);
        }
        if !timeline.copied.is_empty() {
            ui.label(RichText::new(format!("{} copied", timeline.copied.len())).weak().small());
        }
    });
}
