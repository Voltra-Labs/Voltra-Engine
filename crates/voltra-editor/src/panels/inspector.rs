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
use crate::play::PlayState;

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
                // Only while editing, and not while the timeline records:
                // recording keys the primary alone, and while playing the
                // world is the simulation's — writing a value loaded from
                // text would reset the rest's running state (particles,
                // animators, joint impulses).
                let alone =
                    editor.timeline.is_recording() || editor.play.state() != PlayState::Editing;
                let selected: Vec<Entity> = if alone {
                    vec![entity]
                } else {
                    std::iter::once(entity)
                        .chain(
                            editor
                                .selection
                                .iter()
                                .filter(|&e| e != entity && frame.world.is_alive(e)),
                        )
                        .collect()
                };
                let more = match editor.selection.len() {
                    0 | 1 => String::new(),
                    n if alone => format!(" — the last picked of {n}, edited alone"),
                    n => format!(" — {n} selected, the last picked shown"),
                };
                ui.label(format!(
                    "Entity {} (gen {}){more}",
                    entity.index(),
                    entity.generation()
                ))
                .on_hover_text(if selected.len() > 1 {
                    "an edit writes the field changed to every selected entity"
                } else if editor.selection.len() > 1 {
                    "while playing or recording, only this one is edited"
                } else {
                    "the entity the inspector edits"
                });

                // The name, then what it hangs off. Both are identity rather
                // than geometry, so they sit above the components.
                let mut claim: Option<&'static str> = name::show(ui, frame.world, &selected);
                if let Some(parent) = hierarchy::parent_of(frame.world, entity) {
                    ui.label(
                        RichText::new(format!("child of {}", name::of(frame.world, parent))).weak(),
                    );
                }
                ui.separator();
                // Its prefab's buttons act on this entity's instance alone,
                // each as an entry of its own: outside what is written
                // across, which would copy a revert onto the rest.
                prefab::show(editor, ui, frame, entity);
                if !frame.world.is_alive(entity) {
                    return;
                }
                let mut multi = MultiEdit::begin(frame.world, &editor.registry, entity, selected);
                // Touches left by a frame whose sections never ran.
                mixed::take_touched(ui);
                let added = partial::show(ui, frame.world, &editor.registry, &multi);
                if !added.is_empty() {
                    frame.resolve_assets_of(&added);
                    claim = claim.or(Some("Add component"));
                }
                claim = claim.or(sections::show(editor, ui, frame, entity, &mut multi));
                for (name, path, how) in mixed::take_touched(ui) {
                    multi.touch(name, path, how);
                }
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
