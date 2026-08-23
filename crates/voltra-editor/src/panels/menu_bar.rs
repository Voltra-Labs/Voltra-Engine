//! Top menu bar: scene commands and the frame's counters.

use std::path::Path;

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{RigidBody, SceneId};

use crate::editor::Editor;
use crate::play::PlayState;
use crate::spawn;
use crate::undo::{selected_id, SceneView};

/// Where the editor saves when no path has been chosen.
///
/// A parameter with a default rather than a value baked into `save` and `load` —
/// the second caller is a file dialog, and it is already foreseeable.
fn default_path() -> &'static Path {
    Path::new("assets/scenes/scene.ron")
}

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::Panel::top("menu").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Scene", |ui| {
                if ui.button("Spawn sprite").clicked() {
                    spawn::record(editor, frame, "Spawn sprite", |frame| {
                        spawn::sprite(frame, "Sprite", Vec2::ZERO)
                    });
                    ui.close();
                }
                if ui.button("Spawn camera").clicked() {
                    spawn::record(editor, frame, "Spawn camera", |frame| {
                        spawn::camera(frame, "Camera", Vec2::ZERO)
                    });
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
                        selected: selected_id(frame.world, editor.selected),
                    };
                    editor.history.begin("Clear scene", view, ids);

                    for entity in all {
                        frame.world.despawn(entity);
                    }
                    editor.selected = None;

                    let view = SceneView {
                        world: frame.world,
                        registry: &editor.registry,
                        selected: None,
                    };
                    editor.history.commit(view);
                    ui.close();
                }

                ui.separator();

                if ui.button("Save").clicked() {
                    // Stops first so what lands on disk is the authored scene
                    // rather than a mid-flight one, and says so — unlike Clear
                    // and Open, saving is not itself destructive, so a silent
                    // mode change would be a surprise.
                    if editor.stop_play(frame) {
                        log::info!("stopped play so the saved scene is the authored one");
                    }
                    if let Some(parent) = default_path().parent() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            log::error!("could not create {}: {e}", parent.display());
                        }
                    }
                    match voltra_scene::format::save(frame.world, &editor.registry, default_path())
                    {
                        Ok(()) => log::info!("scene saved"),
                        Err(e) => log::error!("could not save the scene: {e}"),
                    }
                    ui.close();
                }

                if ui.button("Open").clicked() {
                    // Silently: Open replaces the world the snapshot belongs
                    // to, so keeping play alive across it is meaningless. This
                    // is Unity's answer — opening a scene exits play mode.
                    editor.stop_play(frame);
                    // Replaces rather than merges: "Open" meaning "add another
                    // copy of everything" would surprise anyone. But the world
                    // is not cleared up front — `load` can fail (missing file,
                    // bad RON, a component that won't parse), and its own
                    // rollback only covers entities it spawned itself; an
                    // entity despawned before the call is outside its reach
                    // and gone for good. So the pre-existing set is captured
                    // first and only despawned once `load` has reported
                    // success, which keeps a failed Open a no-op instead of a
                    // scene-emptying one.
                    //
                    // Between a successful load and the despawn loop below,
                    // both the old and the new scene are live in the world at
                    // once. That is fine for the rest of this frame as long as
                    // nothing in between observes it — no early return, no
                    // `?`, no logging that walks the world — so the despawn
                    // runs unconditionally right after `Ok`.
                    //
                    // If the loaded file has an entity carrying a `SceneId`
                    // that matches one already in the world, the old one is
                    // still in `previous` and gets despawned below, so the
                    // file's copy wins.
                    let previous: Vec<Entity> =
                        frame.world.query::<SceneId>().map(|(e, _)| e).collect();
                    match voltra_scene::format::load(default_path(), &editor.registry, frame.world)
                    {
                        Ok(()) => {
                            for entity in previous {
                                frame.world.despawn(entity);
                            }
                            // The world just changed under every handle in
                            // it: entities from the old scene are gone and
                            // the new ones carry paths nothing has resolved
                            // yet. Runs after the despawns above so it never
                            // wastes a lookup loading a texture for an
                            // entity about to disappear.
                            frame.resolve_scene_assets();
                            // Not load-bearing: a stale `selected` cannot come
                            // back to address a different entity, since
                            // `Entities::is_alive` checks the generation and a
                            // despawn always bumps it. Cleared here only for
                            // consistency with `Clear`, which already does.
                            editor.selected = None;
                            // Every id in the stack belongs to the scene that
                            // was just closed. Unity's answer, and the only
                            // coherent one. Inside the `Ok` arm and only there:
                            // a failed open is a no-op and must leave the
                            // history alone.
                            editor.history.clear();
                            log::info!("scene loaded");
                        }
                        Err(e) => log::error!("could not open the scene: {e}"),
                    }
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

            ui.separator();
            let (width, height) = frame.viewport_size();
            ui.label(format!("viewport {width}x{height}"));
            ui.separator();
            ui.label(format!("{} entities", frame.world.entity_count()));
        });
    });
}
