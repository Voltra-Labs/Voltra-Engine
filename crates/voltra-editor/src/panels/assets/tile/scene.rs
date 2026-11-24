//! A scene's tile: opened as the scene, or edited as a prefab.

use voltra_assets::browse::Entry;
use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;

use super::badge_tile;
use crate::editor::Editor;
use crate::scene_dialogs::{self, Then};

/// A block with a word on it, and the same drag as every loadable tile.
/// Dropped on the viewport, a scene is spawned as a prefab.
///
/// A colour of its own rather than the sound's, so a folder holding both
/// reads at a glance. Not a rendered thumbnail: that would mean spawning
/// every scene on screen into a world of its own to draw it. A file named
/// as a prefab says PREFAB instead, in the selection colour: the same
/// format, meant for a different gesture.
///
/// Double-clicked, a scene opens, through "save changes?" like every other
/// way to Open — Unity's and Godot's gesture — and a file named as a prefab
/// opens in prefab mode over the scene, as Unity's double-click on a prefab
/// does. The menu offers either for any scene, since any can be instanced.
pub(super) fn tile(
    editor: &mut Editor,
    ui: &mut Ui,
    frame: &mut UiFrame<'_>,
    entry: &Entry,
    response: &egui::Response,
) {
    let prefab = crate::prefab::is_prefab_name(&entry.name);
    let (text, fill) = if prefab {
        ("PREFAB", ui.visuals().selection.bg_fill)
    } else {
        ("SCENE", ui.visuals().widgets.active.bg_fill)
    };
    badge_tile(ui, entry, response, text, fill);

    if response.double_clicked() {
        if prefab {
            editor.prefab_mode.request_edit(entry.path.clone());
        } else {
            open(editor, frame, entry);
        }
    }
    response.context_menu(|ui| {
        if ui.button("Open as scene").clicked() {
            open(editor, frame, entry);
            ui.close();
        }
        if ui
            .button("Edit as prefab")
            .on_hover_text("on its own, over the open scene; Back returns to it")
            .clicked()
        {
            editor.prefab_mode.request_edit(entry.path.clone());
            ui.close();
        }
    });
}

/// Opens the scene in place of the open one, through "save changes?".
fn open(editor: &mut Editor, frame: &mut UiFrame<'_>, entry: &Entry) {
    scene_dialogs::request(editor, frame, Then::Open(entry.path.clone()));
}
