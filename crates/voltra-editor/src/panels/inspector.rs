//! Right panel: the components of the selected entity.
//!
//! One file per component's widget, under [`show`], which is only the list of
//! them and the rule that turns an edit into an undo entry.

mod camera;
mod collision;
mod name;
mod path_field;
mod sprite;
mod transform;

use voltra_core::egui::{self, Color32, RichText, Ui};
use voltra_core::UiFrame;
use voltra_scene::{hierarchy, Camera, Collider, SceneId, Sprite, Transform};

use crate::editor::Editor;
use crate::undo::SceneView;

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::Panel::right("inspector")
        .default_size(240.0)
        .show(ui, |ui| {
            ui.heading("Inspector");
            ui.separator();

            // A stale handle is normal, not a bug: the entity may have been
            // despawned since it was selected.
            let Some(entity) = editor.selected.filter(|e| frame.world.is_alive(*e)) else {
                ui.label(RichText::new("nothing selected").italics().weak());
                return;
            };

            // Every `DragValue` below gets its id from layout position, not from
            // `entity`. Without this, selecting a different sprite that lands the
            // same row on screen reuses the previous widget's id, and egui — which
            // still thinks that id is mid-edit — applies the leftover text to the
            // new entity instead of the one the user clicked. Salting the whole
            // body by `entity` makes every id here a function of the entity too, so
            // a selection change is a real id change and egui starts clean.
            ui.push_id(entity, |ui| {
                ui.label(format!(
                    "Entity {} (gen {})",
                    entity.index(),
                    entity.generation()
                ));

                // The name, then what it hangs off. Both are identity rather
                // than geometry, so they sit above the components.
                let mut claim: Option<&'static str> = name::show(ui, frame.world, entity);
                if let Some(parent) = hierarchy::parent_of(frame.world, entity) {
                    ui.label(
                        RichText::new(format!("child of {}", name::of(frame.world, parent))).weak(),
                    );
                }
                ui.separator();
                if let Some(transform) = frame.world.get_mut::<Transform>(entity) {
                    claim = transform::show(ui, transform);
                }
                if let Some(sprite) = frame.world.get_mut::<Sprite>(entity) {
                    ui.separator();
                    claim = claim.or(sprite::show(
                        ui,
                        sprite,
                        frame.textures,
                        frame.device,
                        frame.queue,
                    ));
                }
                if let Some(camera) = frame.world.get_mut::<Camera>(entity) {
                    ui.separator();
                    claim = claim.or(camera::show(ui, camera));
                }
                // Keyed off the collider rather than off the filter itself:
                // both filter components are absent by default, and an entity
                // whose filter cannot be reached until it has one would never
                // get its first.
                if frame.world.get::<Collider>(entity).is_some() {
                    ui.separator();
                    claim = claim.or(collision::show(ui, frame.world, entity));
                }
                // A field held down or focused keeps one entry open for the
                // whole edit, the way a gizmo drag does. Collected and applied
                // here rather than pushed from inside the widget helpers: they
                // hold a `&mut` borrowed out of the world, and the history has
                // to read the world.
                if let Some(label) = claim {
                    editor.history.claim(label);
                }

                ui.separator();
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
            });
        });
}

/// What to call an edit made through this widget this frame, if it is live.
///
/// Dragged *or* focused: a `DragValue` can also be clicked into and typed in,
/// and an entry that closed between two keystrokes would be one undo per
/// character.
pub(super) fn active(response: &egui::Response, label: &'static str) -> Option<&'static str> {
    (response.dragged() || response.has_focus()).then_some(label)
}
