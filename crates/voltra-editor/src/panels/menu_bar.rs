//! Top menu bar: scene commands and the frame's counters.

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{RigidBody, SceneId};

use crate::editor::Editor;
use crate::play::PlayState;
use crate::spawn;

mod game_menu;
mod spawn_items;
use crate::undo::SceneView;

use crate::scene_dialogs::{self, Then};

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::Panel::top("menu").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Scene", |ui| {
                spawn_items::show(editor, ui, frame);
                let selected = !editor.selection.is_empty();
                if ui
                    .add_enabled(selected, egui::Button::new("Create prefab from selection"))
                    .on_disabled_hover_text("select an entity first")
                    .clicked()
                {
                    crate::prefab::create_from_selection(editor, frame);
                    ui.close();
                }
                if ui.button("Clear").clicked() {
                    // Silently, because Clear is destructive anyway: a scene
                    // being emptied has no mid-flight state worth protecting.
                    editor.stop_play(frame);
                    // Predicate is identity, not appearance: `SceneId` is what
                    // makes an entity part of the scene — the same rule `save`
                    // uses to decide what gets written. Querying `Sprite`
                    // instead would miss an identity-carrying entity with no
                    // `Sprite` (loadable from a file, just not this build's
                    // demo/menu spawners) and leave it behind after a Clear
                    // that looked complete, only for it to resurface on the
                    // next Save. A `Sprite` with no `SceneId` is deliberately
                    // transient and Clear should leave it alone, which this
                    // predicate does for free.
                    let all: Vec<Entity> = frame.world.query::<SceneId>().map(|(e, _)| e).collect();
                    // One entry for the whole Clear, not one per entity: the
                    // edit is the command, and undoing it has to bring the
                    // scene back rather than a sprite at a time.
                    let ids: Vec<SceneId> = all
                        .iter()
                        .filter_map(|e| frame.world.get::<SceneId>(*e).copied())
                        .collect();
                    let view = SceneView {
                        world: frame.world,
                        registry: &editor.registry,
                        selected: editor.selection.ids(frame.world),
                    };
                    editor.history.begin("Clear scene", view, ids);

                    for entity in all {
                        frame.world.despawn(entity);
                    }
                    editor.selection.clear();

                    let view = SceneView {
                        world: frame.world,
                        registry: &editor.registry,
                        selected: Vec::new(),
                    };
                    editor.history.commit(view);
                    ui.close();
                }

                ui.separator();

                let item = |text: &str, keys: &str| egui::Button::new(text).shortcut_text(keys);
                if ui.add(item("New", "Ctrl+N")).clicked() {
                    scene_dialogs::request(editor, frame, Then::New);
                    ui.close();
                }
                if ui.add(item("Open…", "Ctrl+O")).clicked() {
                    scene_dialogs::show_open(editor, frame);
                    ui.close();
                }
                if ui.add(item("Save", "Ctrl+S")).clicked() {
                    scene_dialogs::save(editor, frame);
                    ui.close();
                }
                if ui.add(item("Save As…", "Ctrl+Shift+S")).clicked() {
                    scene_dialogs::show_save_as(editor, None);
                    ui.close();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    scene_dialogs::request(editor, frame, Then::Quit);
                    ui.close();
                }
            });

            ui.menu_button("Edit", |ui| {
                // Disabled rather than hidden while playing, and disabled
                // rather than absent when there is nothing to undo: a menu item
                // that comes and goes is harder to find than one that is greyed
                // out, and the label is where the user reads what would happen.
                let editing = editor.play.state() == PlayState::Editing;

                let undo = editor
                    .history
                    .undo_label()
                    .map_or_else(|| "Undo".to_owned(), |label| format!("Undo {label}"));
                if ui
                    .add_enabled(
                        editing && !editor.history.is_empty(),
                        egui::Button::new(undo).shortcut_text("Ctrl+Z"),
                    )
                    .clicked()
                {
                    editor.undo(frame);
                    ui.close();
                }

                let redo = editor
                    .history
                    .redo_label()
                    .map_or_else(|| "Redo".to_owned(), |label| format!("Redo {label}"));
                if ui
                    .add_enabled(
                        editing && editor.history.redo_len() > 0,
                        egui::Button::new(redo).shortcut_text("Ctrl+Y"),
                    )
                    .clicked()
                {
                    editor.redo(frame);
                    ui.close();
                }

                // What acts on the selection. Paste is the key alone: the
                // clipboard's text reaches the editor only with the key.
                ui.separator();
                let some = editing && !editor.selection.is_empty();
                let item = |text: &str, keys: &str| egui::Button::new(text).shortcut_text(keys);
                if ui.add_enabled(some, item("Duplicate", "Ctrl+D")).clicked() {
                    crate::clipboard::duplicate(editor, frame);
                    ui.close();
                }
                if ui.add_enabled(some, item("Copy", "Ctrl+C")).clicked() {
                    crate::clipboard::copy_selection(editor, ui.ctx(), frame);
                    ui.close();
                }
                if ui.add_enabled(some, item("Delete", "Del")).clicked() {
                    crate::delete::selection(editor, frame);
                    ui.close();
                }
            });

            ui.menu_button("Physics", |ui| {
                ui.checkbox(&mut editor.show_colliders, "Show colliders");

                ui.separator();

                // The only way to author a body today: the inspector edits a
                // `Transform` and a `Sprite` and nothing else yet. Two
                // spawners rather than one because a falling body on its own
                // demonstrates gravity but nothing about detection — the
                // contact needs something to be against.
                // Tilted, so the drop shows what 11b-3 added: the box lands on
                // a corner, tips onto a face and stops. Level, it would look
                // exactly like the stage before it. The inspector edits the
                // rotation afterwards like any other field.
                if ui.button("Spawn falling body").clicked() {
                    spawn::record(editor, frame, "Spawn body", |frame| {
                        spawn::body(
                            frame,
                            "Falling body",
                            Vec2::new(0.0, 1.5),
                            Vec2::splat(0.4),
                            0.3,
                            RigidBody::new_dynamic(1.0),
                            [1.0, 0.8, 0.3, 1.0],
                        )
                    });
                    ui.close();
                }
                if ui.button("Spawn pendulum").clicked() {
                    crate::joints::spawn_pendulum(editor, frame);
                    ui.close();
                }
                let joinable = crate::joints::can_join(editor, frame.world);
                if ui
                    .add_enabled(joinable, egui::Button::new("Join selection"))
                    .on_hover_text("a hinge at the last picked body: to the other selected body, or to the world")
                    .on_disabled_hover_text("select a body, and at most one other")
                    .clicked()
                {
                    crate::joints::join_selection(editor, frame);
                    ui.close();
                }
                if ui.button("Spawn floor").clicked() {
                    spawn::record(editor, frame, "Spawn floor", |frame| {
                        spawn::body(
                            frame,
                            "Floor",
                            Vec2::new(0.0, -1.2),
                            Vec2::new(3.0, 0.3),
                            0.0,
                            RigidBody::new_static(),
                            [0.5, 0.5, 0.55, 1.0],
                        )
                    });
                    ui.close();
                }
            });

            game_menu::show(editor, ui, frame);

            ui.menu_button("Window", |ui| {
                if ui
                    .add_enabled(!editor.profiler.is_open(), egui::Button::new("Profiler"))
                    .clicked()
                {
                    editor.profiler.open();
                    ui.close();
                }
            });

            ui.separator();
            let (width, height) = frame.viewport_size();
            ui.label(format!("viewport {width}x{height}"));
            ui.separator();
            ui.label(format!("{} entities", frame.world.entity_count()));
        });
    });
}
