//! Bottom panel: the clip the selected entity's animation player plays —
//! its tracks, their keys on a time strip, the playhead, and Save.
//!
//! Shown while editing, for a primary selection with a player naming a
//! clip. A clip is an asset, not the scene: its edits are not in the undo
//! history but held in the store — played at once by every player of it —
//! until Save writes them or Revert reads the file back. Unity's Animation
//! window, cut to keys and eases.

mod add_track;
mod header;
mod key;
mod ruler;
mod strip;

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;
use voltra_scene::AnimationPlayer;

use crate::editor::Editor;
use crate::play::PlayState;

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    if editor.play.state() != PlayState::Editing {
        return;
    }
    let Some(root) = editor.selection.live_primary(frame.world) else {
        return;
    };
    let Some(path) = frame
        .world
        .get::<AnimationPlayer>(root)
        .and_then(|player| player.clip.clone())
    else {
        return;
    };
    let timeline = &mut editor.timeline;
    timeline.open(&path);
    let handle = frame.animation_clips.load(&path);

    egui::Panel::bottom("timeline")
        .resizable(true)
        .default_size(170.0)
        .show(ui, |ui| {
            header::show(ui, timeline, frame, &path, handle);
            ui.separator();
            let clip = frame.animation_clips.get_mut(handle);
            let mut changed = false;
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    changed |= strip::show(ui, timeline, frame.world, root, clip);
                    ui.separator();
                    changed |= key::show(ui, timeline, clip);
                    changed |= add_track::show(ui, timeline, frame.world, root, clip);
                });
            if changed {
                timeline.dirty.insert(path.clone());
            }
        });
}
