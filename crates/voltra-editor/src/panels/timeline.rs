//! Bottom panel: the clip the selected entity's animation player plays —
//! its tracks, their keys on a time strip, the playhead, and Save.
//!
//! Shown while editing, for a primary selection with a player naming a
//! clip. A clip is an asset, not the scene: its edits are not in the undo
//! history but held in the store — played at once by every player of it —
//! until Save writes them or Revert reads the file back. Unity's Animation
//! window, cut to keys and eases.

mod add_track;
mod key;
mod ruler;
mod strip;

use voltra_assets::{AssetPath, Handle};
use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_core::UiFrame;
use voltra_scene::{AnimationClip, AnimationPlayer};

use crate::editor::Editor;
use crate::play::PlayState;
use crate::timeline::Timeline;

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
            header(ui, timeline, frame, &path, handle);
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

/// The clip's name and settings, the playhead, the preview switch, and the
/// file's two buttons.
fn header(
    ui: &mut Ui,
    timeline: &mut Timeline,
    frame: &mut UiFrame<'_>,
    path: &AssetPath,
    handle: Handle<AnimationClip>,
) {
    let dirty = timeline.dirty.contains(path);
    ui.horizontal(|ui| {
        ui.heading("Timeline");
        let name = if dirty {
            format!("{} •", path.as_str())
        } else {
            path.as_str().to_owned()
        };
        ui.label(RichText::new(name).weak())
            .on_hover_text("• edited since the file was saved");

        let clip = frame.animation_clips.get_mut(handle);
        if ui.checkbox(&mut clip.looping, "loop").changed() {
            timeline.dirty.insert(path.clone());
        }
        ui.label("length")
            .on_hover_text("seconds; 0 ends on the last key");
        if ui
            .add(
                DragValue::new(&mut clip.length)
                    .speed(0.01)
                    .range(0.0..=3600.0),
            )
            .changed()
        {
            timeline.dirty.insert(path.clone());
        }
        let end = clip.duration();
        ui.label("at");
        ui.add(
            DragValue::new(&mut timeline.playhead)
                .speed(0.01)
                .range(0.0..=end.max(1.0))
                .suffix("s"),
        );
        ui.toggle_value(&mut timeline.preview, "Preview")
            .on_hover_text(
                "shows the clip at the playhead in the viewport; the scene keeps its own values",
            );

        if ui.add_enabled(dirty, egui::Button::new("Save")).clicked() {
            match frame.animation_clips.save(path) {
                Ok(()) => {
                    timeline.dirty.remove(path);
                    log::info!("saved {}", path.as_str());
                }
                Err(e) => log::error!("could not save {}: {e}", path.as_str()),
            }
        }
        if ui
            .add_enabled(dirty, egui::Button::new("Revert"))
            .on_hover_text("reads the file back, dropping the edits")
            .clicked()
        {
            if frame.animation_clips.reload(path) {
                timeline.dirty.remove(path);
                timeline.selected = None;
            } else {
                log::warn!("{} could not be read back; the edits stay", path.as_str());
            }
        }
    });
}
