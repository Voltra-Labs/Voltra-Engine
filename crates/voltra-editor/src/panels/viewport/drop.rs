//! Dropping an asset from the browser onto the viewport.

use voltra_assets::AssetPath;
use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;

use crate::drag;
use crate::editor::Editor;
use crate::play::PlayState;
use crate::spawn;

/// Adds what an asset makes — a sprite, a sound, a prefab's instance — where
/// it was dropped, as one undo entry.
///
/// Unity, Unreal and Godot all instantiate on drop rather than opening a
/// dialogue, and all three put the new object where the pointer released rather
/// than at the origin — the drop *is* the placement, and a sprite that appears
/// somewhere else has to be moved for no reason.
///
/// Refused while playing, like every other edit: the scene the history is
/// addressed against is the authored one, and Stop would throw the sprite away
/// without saying so.
pub(super) fn drop_asset(
    editor: &mut Editor,
    frame: &mut UiFrame<'_>,
    ui: &Ui,
    scene: &egui::Response,
) {
    if editor.play.state() != PlayState::Editing {
        return;
    }

    // What the drop would do, before the release. A viewport that accepts a
    // drag silently and only reacts once it is too late to change your mind is
    // the complaint every content browser gets.
    if scene.dnd_hover_payload::<AssetPath>().is_some() {
        drag::outline(ui, scene.rect, ui.visuals().selection.bg_fill);
    }

    let Some(path) = scene.dnd_release_payload::<AssetPath>() else {
        return;
    };
    // `Response::interact_pointer_pos` is `None` on the release frame of a drag
    // that started in another widget, which is every drop this handles.
    let Some(pointer) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    let at = crate::picking::world_at(scene, pointer, frame);

    // A scene is a prefab: the drop spawns a copy of everything in it.
    if voltra_assets::watch::is_scene(path.as_str()) {
        spawn::record_all(editor, frame, "Add prefab", |frame, registry| {
            spawn::prefab(frame, registry, at, &path)
        });
        return;
    }

    // A sound is not a sprite at all: it becomes something that plays it.
    if voltra_assets::watch::is_clip(path.as_str()) {
        spawn::record(editor, frame, "Add audio source", |frame| {
            spawn::audio_source(frame, at, &path)
        });
        return;
    }

    // A face becomes a label written in it.
    if voltra_assets::watch::is_font(path.as_str()) {
        spawn::record(editor, frame, "Add text", |frame| {
            spawn::text(frame, "Text", at, Some(&path))
        });
        return;
    }

    // Which of the two the drop means is the file it names: a slicing brings
    // its own sheet with it, a bare image is the sheet.
    let sliced = voltra_assets::atlas::is_atlas(path.as_str());
    spawn::record(editor, frame, "Add sprite", |frame| {
        if sliced {
            spawn::atlas_sprite(frame, at, &path)
        } else {
            spawn::textured_sprite(frame, at, &path)
        }
    });
}
