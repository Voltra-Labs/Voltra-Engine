//! What a prefab instance's entity overrides, and the buttons that take it
//! back to the prefab or write it over it.

use voltra_core::egui::{RichText, Ui};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_scene::{prefab, PrefabInstance, PrefabPart};

use crate::editor::Editor;

/// The section, for an entity that is part of an instance; nothing for one
/// that is not. Its buttons act at once, each as its own undo entry.
pub(super) fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>, entity: Entity) {
    let Some(root) = prefab::instance_root(frame.world, entity) else {
        return;
    };
    let Some(source) = frame
        .world
        .get::<PrefabInstance>(root)
        .map(|l| l.source.clone())
    else {
        return;
    };
    let overridden: Vec<String> = frame
        .world
        .get::<PrefabPart>(entity)
        .map(|part| part.overridden.iter().cloned().collect())
        .unwrap_or_default();

    ui.label(RichText::new("Prefab").strong()).on_hover_text(
        "kept in step with its file: what it does not override follows every edit to it",
    );
    if root == entity {
        ui.label(RichText::new(format!("instance of {}", source.as_str())).weak());
    } else {
        ui.label(RichText::new(format!("part of an instance of {}", source.as_str())).weak());
    }
    if frame.world.get::<PrefabPart>(entity).is_none() {
        ui.label(RichText::new("added to the instance: its own").weak());
    }
    if ui
        .button("Edit prefab")
        .on_hover_text("open the prefab on its own; Back returns here, and this instance follows")
        .clicked()
    {
        // Next frame: the rest of this inspector still draws this entity.
        editor.prefab_mode.request_edit(source.clone());
    }

    let mut revert = None;
    for name in &overridden {
        ui.horizontal(|ui| {
            ui.label(format!("overrides {name}"));
            if ui
                .small_button("Revert")
                .on_hover_text("back to the prefab's")
                .clicked()
            {
                revert = Some(name.clone());
            }
        });
    }
    if let Some(name) = revert {
        // A prefab reads, and is read onto, the authored scene, not a pose.
        editor
            .timeline
            .end_record(frame.world, frame.animation_clips);
        crate::instance::revert(editor, frame, entity, Some(&name));
    }

    if root == entity {
        ui.horizontal(|ui| {
            if ui
                .button("Apply to prefab")
                .on_hover_text("write this instance over its file; every other instance follows")
                .clicked()
            {
                editor
                    .timeline
                    .end_record(frame.world, frame.animation_clips);
                crate::instance::apply(editor, frame, root);
            }
            if ui
                .button("Revert instance")
                .on_hover_text("every override and every deleted part back to the prefab's")
                .clicked()
            {
                editor
                    .timeline
                    .end_record(frame.world, frame.animation_clips);
                crate::instance::revert_instance(editor, frame, root);
            }
        });
    }
    ui.separator();
}
