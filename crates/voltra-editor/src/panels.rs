//! The editor's panels and the selection they share.

use voltra_core::egui::{self, Color32, DragValue, RichText, Ui};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{Sprite, Transform};

/// Editor state that outlives a frame.
///
/// egui is immediate mode and remembers nothing between frames, so anything the
/// editor still needs next frame — the selection — has to be kept here.
#[derive(Default)]
pub struct Editor {
    selected: Option<Entity>,
}

impl Editor {
    /// Lays out the whole editor. Called once per frame with the root `Ui`.
    pub fn ui(&mut self, ui: &mut Ui, frame: &mut UiFrame<'_>) {
        self.menu_bar(ui, frame);
        self.hierarchy(ui, frame);
        self.inspector(ui, frame);
        // Last, so it takes whatever room the docked panels left rather than
        // the other way round.
        self.viewport(ui, frame);
    }

    fn menu_bar(&mut self, ui: &mut Ui, frame: &mut UiFrame<'_>) {
        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Scene", |ui| {
                    if ui.button("Spawn sprite").clicked() {
                        self.selected = Some(spawn_sprite(frame));
                        ui.close();
                    }
                    if ui.button("Clear").clicked() {
                        let all: Vec<Entity> =
                            frame.world.query::<Sprite>().map(|(e, _)| e).collect();
                        for entity in all {
                            frame.world.despawn(entity);
                        }
                        self.selected = None;
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

    fn hierarchy(&mut self, ui: &mut Ui, frame: &mut UiFrame<'_>) {
        egui::Panel::left("hierarchy")
            .default_size(200.0)
            .show(ui, |ui| {
                ui.heading("Hierarchy");
                ui.separator();

                // Collected before the loop: selecting inside it would hold a
                // borrow of the world while the buttons want to mutate it.
                let entities: Vec<Entity> = frame.world.query::<Sprite>().map(|(e, _)| e).collect();

                if entities.is_empty() {
                    ui.label(RichText::new("nothing in the scene").italics().weak());
                    return;
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for entity in entities {
                        let label = format!("Entity {}", entity.index());
                        if ui
                            .selectable_label(self.selected == Some(entity), label)
                            .clicked()
                        {
                            self.selected = Some(entity);
                        }
                    }
                });
            });
    }

    fn inspector(&mut self, ui: &mut Ui, frame: &mut UiFrame<'_>) {
        egui::Panel::right("inspector")
            .default_size(240.0)
            .show(ui, |ui| {
                ui.heading("Inspector");
                ui.separator();

                // A stale handle is normal, not a bug: the entity may have been
                // despawned since it was selected.
                let Some(entity) = self.selected.filter(|e| frame.world.is_alive(*e)) else {
                    ui.label(RichText::new("nothing selected").italics().weak());
                    return;
                };

                ui.label(format!(
                    "Entity {} (gen {})",
                    entity.index(),
                    entity.generation()
                ));
                ui.separator();

                if let Some(transform) = frame.world.get_mut::<Transform>(entity) {
                    transform_ui(ui, transform);
                }
                if let Some(sprite) = frame.world.get_mut::<Sprite>(entity) {
                    ui.separator();
                    sprite_ui(ui, sprite);
                }

                ui.separator();
                if ui
                    .button(RichText::new("Delete").color(Color32::LIGHT_RED))
                    .clicked()
                {
                    frame.world.despawn(entity);
                    self.selected = None;
                }
            });
    }

    fn viewport(&mut self, ui: &mut Ui, frame: &mut UiFrame<'_>) {
        egui::CentralPanel::default()
            // The scene brings its own background; the panel's would only show
            // as a border around the image.
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let available = ui.available_size();
                // egui lays out in logical points while the target is sized in
                // physical pixels. Missing this conversion renders the scene at
                // half resolution on a 200% display.
                let scale = ui.ctx().pixels_per_point();
                frame.request_viewport_size(
                    (available.x * scale) as u32,
                    (available.y * scale) as u32,
                );

                ui.add(egui::Image::new(egui::load::SizedTexture::new(
                    frame.viewport(),
                    available,
                )));
            });
    }
}

fn transform_ui(ui: &mut Ui, transform: &mut Transform) {
    ui.label(RichText::new("Transform").strong());

    egui::Grid::new("transform").num_columns(2).show(ui, |ui| {
        ui.label("position");
        ui.horizontal(|ui| {
            ui.add(DragValue::new(&mut transform.translation.x).speed(0.01));
            ui.add(DragValue::new(&mut transform.translation.y).speed(0.01));
        });
        ui.end_row();

        ui.label("rotation");
        ui.drag_angle(&mut transform.rotation);
        ui.end_row();

        ui.label("scale");
        ui.horizontal(|ui| {
            ui.add(DragValue::new(&mut transform.scale.x).speed(0.01));
            ui.add(DragValue::new(&mut transform.scale.y).speed(0.01));
        });
        ui.end_row();
    });
}

fn sprite_ui(ui: &mut Ui, sprite: &mut Sprite) {
    ui.label(RichText::new("Sprite").strong());
    ui.horizontal(|ui| {
        ui.label("colour");
        ui.color_edit_button_rgba_unmultiplied(&mut sprite.color);
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
