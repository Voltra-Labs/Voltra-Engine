//! Top menu bar: scene commands and the frame's counters.

use std::path::Path;

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{ComponentRegistry, SceneId, Sprite, Transform};

use crate::editor::Editor;

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
                    editor.selected = Some(spawn_sprite(frame));
                    ui.close();
                }
                if ui.button("Clear").clicked() {
                    let all: Vec<Entity> = frame.world.query::<Sprite>().map(|(e, _)| e).collect();
                    for entity in all {
                        frame.world.despawn(entity);
                    }
                    editor.selected = None;
                    ui.close();
                }

                ui.separator();

                if ui.button("Save").clicked() {
                    let registry = ComponentRegistry::with_defaults();
                    if let Some(parent) = default_path().parent() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            log::error!("could not create {}: {e}", parent.display());
                        }
                    }
                    match voltra_scene::format::save(frame.world, &registry, default_path()) {
                        Ok(()) => log::info!("scene saved"),
                        Err(e) => log::error!("could not save the scene: {e}"),
                    }
                    ui.close();
                }

                if ui.button("Open").clicked() {
                    let registry = ComponentRegistry::with_defaults();
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
                    match voltra_scene::format::load(default_path(), &registry, frame.world) {
                        Ok(()) => {
                            for entity in previous {
                                frame.world.despawn(entity);
                            }
                            // Not load-bearing: a stale `selected` cannot come
                            // back to address a different entity, since
                            // `Entities::is_alive` checks the generation and a
                            // despawn always bumps it. Cleared here only for
                            // consistency with `Clear`, which already does.
                            editor.selected = None;
                            log::info!("scene loaded");
                        }
                        Err(e) => log::error!("could not open the scene: {e}"),
                    }
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

/// Drops a white unit sprite at the origin, ready to be moved.
fn spawn_sprite(frame: &mut UiFrame<'_>) -> Entity {
    let entity = frame.world.spawn();
    frame.world.insert(entity, SceneId::new());
    frame
        .world
        .insert(entity, Transform::default().with_scale(Vec2::splat(0.4)));
    frame.world.insert(entity, Sprite::default());
    entity
}
