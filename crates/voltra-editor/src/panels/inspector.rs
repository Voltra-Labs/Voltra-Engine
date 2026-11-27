//! Right panel: the components of the selected entity.
//!
//! One file per component's widget, under [`show`], which is only the list of
//! them and the rule that turns an edit into an undo entry.

mod animation;
mod animation_player;
mod animator;
mod audio_source;
mod camera;
mod camera_rig;
mod camera_view;
mod character;
mod collider;
mod collision;
mod delete;
mod game_components;
mod global_light;
mod joint;
mod light;
mod light_layers;
mod mixed;
mod name;
mod parallax;
mod partial;
mod particles;
mod path_field;
mod prefab;
mod sections;
mod shadow_caster;
mod shapes2d;
mod skeleton;
mod skinned;
mod sorting_layer;
mod sprite;
mod sprite_material;
mod text;
mod tilemap;
mod transform;
mod ui_button;
mod ui_controls;
mod ui_dropdown;
mod ui_rect;
mod ui_scroll;
mod ui_slice;
mod ui_slider;
mod ui_stack;
mod ui_tabs;
mod ui_text_field;
mod ui_toggle;
mod ui_tooltip;

use voltra_core::egui::{self, RichText, Ui};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_scene::hierarchy;

use crate::editor::Editor;
use crate::multi_edit::MultiEdit;

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::Panel::right("inspector")
        .default_size(240.0)
        .show(ui, |ui| {
            ui.heading("Inspector");
            ui.separator();

            // A stale handle is normal, not a bug: the entity may have been
            // despawned since it was selected.
            let Some(entity) = editor.selection.live_primary(frame.world) else {
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
                // While the timeline records, only the primary is keyed: the
                // rest of the selection is not written to behind its back.
                let recording = editor.timeline.is_recording();
                let selected: Vec<Entity> = if recording {
                    Vec::new()
                } else {
                    editor.selection.iter().collect()
                };
                let mut multi = MultiEdit::begin(frame.world, &editor.registry, entity, selected);
                let more = match editor.selection.len() {
                    0 | 1 => String::new(),
                    n if recording => format!(" — the last picked of {n}, keyed alone"),
                    n => format!(" — {n} selected, the last picked shown"),
                };
                ui.label(format!(
                    "Entity {} (gen {}){more}",
                    entity.index(),
                    entity.generation()
                ))
                .on_hover_text(if multi.is_multi() {
                    "an edit writes the field changed to every selected entity"
                } else {
                    "the entity the inspector edits"
                });

                // The name, then what it hangs off. Both are identity rather
                // than geometry, so they sit above the components.
                let named: Vec<Entity> = multi.all().collect();
                let mut claim: Option<&'static str> = name::show(ui, frame.world, &named);
                if let Some(parent) = hierarchy::parent_of(frame.world, entity) {
                    ui.label(
                        RichText::new(format!("child of {}", name::of(frame.world, parent))).weak(),
                    );
                }
                ui.separator();
                let added = partial::show(ui, frame.world, &editor.registry, &multi);
                if !added.is_empty() {
                    frame.resolve_assets_of(&added);
                    claim = claim.or(Some("Add component"));
                }
                claim = claim.or(sections::show(editor, ui, frame, entity, &mut multi));
                // What the sections did to the primary, written into the rest
                // of the selection before the history takes the frame's after
                // side: one entry for all of them.
                let written = multi.finish(frame.world, &editor.registry);
                // Loaded from text, the rest carry no handles yet.
                frame.resolve_assets_of(&written);
                // A field held down or focused keeps one entry open for the
                // whole edit, the way a gizmo drag does. Collected and applied
                // here rather than pushed from inside the widget helpers: they
                // hold a `&mut` borrowed out of the world, and the history has
                // to read the world.
                if let Some(label) = claim {
                    editor.history.claim(label);
                }

                ui.separator();
                delete::show(editor, ui, frame);
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
