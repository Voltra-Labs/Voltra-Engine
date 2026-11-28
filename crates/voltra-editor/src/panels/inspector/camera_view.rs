//! Where a camera draws, 2D or 3D: its viewport, a render texture in
//! place of the screen, and for a 2D one pixel-perfect snapping and
//! whether it turns with its parent.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::{Camera2D, Camera3D, CameraTarget, Viewport};

use super::active;
use super::path_field::{self, PathEdit};

pub(super) fn show(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    let mut claim = None;
    let viewport = match world.get_mut::<Camera2D>(entity) {
        Some(camera) => Some(&mut camera.viewport),
        None => world
            .get_mut::<Camera3D>(entity)
            .map(|camera| &mut camera.viewport),
    };
    if let Some(viewport) = viewport {
        ui.label(RichText::new("Draws into").strong())
            .on_hover_text(
                "the part of the screen, or a render texture, this camera draws; cameras draw \
             in priority order, 3D before 2D at a tie, a later one over an earlier",
            );
        egui::Grid::new("camera_view")
            .num_columns(2)
            .show(ui, |ui| claim = viewport_rows(ui, viewport));
    }
    if let Some(camera) = world.get_mut::<Camera2D>(entity) {
        egui::Grid::new("camera_view_2d")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("pixel perfect").on_hover_text(
                    "snap the view to whole pixels: pair with nearest filtering; off while the view is turned",
                );
                if ui.checkbox(&mut camera.pixel_perfect, "").changed() {
                    claim = claim.or(Some("Toggle pixel perfect"));
                }
                ui.end_row();
                ui.label("turn with parent").on_hover_text(
                    "the view turns with its parents too, not only with its own rotation",
                );
                if ui.checkbox(&mut camera.inherit_rotation, "").changed() {
                    claim = claim.or(Some("Toggle camera turn with parent"));
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

/// The viewport's four fractions and the presets, as grid rows.
fn viewport_rows(ui: &mut Ui, viewport: &mut Viewport) -> Option<&'static str> {
    let mut claim = None;
    ui.label("viewport")
        .on_hover_text("x, y, width, height as fractions of the screen, from its top-left");
    ui.horizontal(|ui| {
        let v = &mut *viewport;
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
                *viewport = preset;
                claim = Some("Set camera viewport");
            }
        }
    });
    ui.end_row();
    claim
}
