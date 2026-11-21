//! The selection's Delete button, and the undo entry it records.

use voltra_core::egui::{Color32, RichText, Ui};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_scene::{hierarchy, SceneId};

use crate::editor::Editor;
use crate::undo::SceneView;

/// Deletes `entity` and everything under it when the button is clicked.
pub(super) fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>, entity: Entity) {
    if ui
        .button(RichText::new("Delete").color(Color32::LIGHT_RED))
        .clicked()
    {
        // Explicit rather than watched: the watcher's `after` is
        // read at the end of the frame from an entity that no longer
        // exists, which is right, but the selection has to be
        // recorded as it was *before* the despawn cleared it.
        let id = frame.world.get::<SceneId>(entity).copied();
        // Everything under it goes too, so everything under it has
        // to be in the entry — an undo that revived the parent
        // alone would leave its children deleted for good.
        let going: Vec<SceneId> = id
            .into_iter()
            .chain(
                hierarchy::descendants(frame.world, entity)
                    .into_iter()
                    .filter_map(|e| frame.world.get::<SceneId>(e).copied()),
            )
            .collect();
        let view = SceneView {
            world: frame.world,
            registry: &editor.registry,
            selected: id,
        };
        editor.history.begin("Delete", view, going);

        hierarchy::despawn_recursive(frame.world, entity);
        editor.selected = None;

        let view = SceneView {
            world: frame.world,
            registry: &editor.registry,
            selected: None,
        };
        editor.history.commit(view);
    }
}
