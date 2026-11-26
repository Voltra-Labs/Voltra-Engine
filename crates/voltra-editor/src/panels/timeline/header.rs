//! The timeline's top row: the clip's name and settings, the playhead, the
//! preview switch, and the file's two buttons.

use voltra_assets::{AssetPath, Handle};
use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_core::UiFrame;
use voltra_scene::AnimationClip;

use crate::timeline::Timeline;

/// The clip's name and settings, the playhead, the preview switch, and the
/// file's two buttons.
pub(super) fn show(
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
