//! Where a camera draws: its viewport, pixel-perfect snapping, and a render
//! texture in place of the screen.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::{Camera, CameraTarget, Viewport};

use super::active;
use super::path_field::{self, PathEdit};

pub(super) fn show(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    let mut claim = None;
    if let Some(camera) = world.get_mut::<Camera>(entity) {
        ui.label(RichText::new("Draws into").strong())
            .on_hover_text("the part of the screen, or a render texture, this camera draws");
        egui::Grid::new("camera_view")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("viewport").on_hover_text(
                    "x, y, width, height as fractions of the screen, from its top-left",
                );
                ui.horizontal(|ui| {
                    let v = &mut camera.viewport;
                    for value in [&mut v.x, &mut v.y, &mut v.width, &mut v.height] {
                        let drag = DragValue::new(value).speed(0.005).range(0.0..=1.0);
                        claim = claim.or(active(&ui.add(drag), "Set camera viewport"));
                    }
                });
                ui.end_row();
                ui.label("");
                ui.horizontal(|ui| {
                    for (label, preset) in [
                        ("full", Viewport::FULL),
                        ("left", Viewport::LEFT),
                        ("right", Viewport::RIGHT),
                    ] {
                        if ui.small_button(label).clicked() {
                            camera.viewport = preset;
                            claim = Some("Set camera viewport");
                        }
                    }
                });
                ui.end_row();
                ui.label("pixel perfect")
                    .on_hover_text("snap the view to whole pixels: pair with nearest filtering");
                if ui.checkbox(&mut camera.pixel_perfect, "").changed() {
                    claim = claim.or(Some("Toggle pixel perfect"));
                }
                ui.end_row();
            });
    }
    let committed = world
        .get::<CameraTarget>(entity)
        .map(|t| t.texture.as_str().to_owned())
        .unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("render texture")
            .on_hover_text("a .rendertexture.ron to draw into instead of the screen");
    });
    match path_field::show(ui, "camera_target", "x.rendertexture.ron", &committed) {
        Some(PathEdit::Cleared) => {
            world.remove::<CameraTarget>(entity);
            claim = Some("Clear camera target");
        }
        Some(PathEdit::Set(path)) => {
            world.insert(entity, CameraTarget::new(path));
            claim = Some("Set camera target");
        }
        None => {}
    }
    claim
}
