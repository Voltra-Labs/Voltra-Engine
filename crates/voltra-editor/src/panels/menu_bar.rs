//! Top menu bar: scene commands and the frame's counters.

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{Sprite, Transform};

use crate::editor::Editor;

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
    frame
        .world
        .insert(entity, Transform::default().with_scale(Vec2::splat(0.4)));
    frame.world.insert(entity, Sprite::default());
    entity
}
