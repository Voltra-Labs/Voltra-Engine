//! A camera's rig: what it follows and how, and how it shakes.

mod follow;
mod shake;

use voltra_core::egui::{DragValue, Ui};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{Camera2D, CameraFollow2D, CameraShake};

use super::active;

/// The follow and shake sections, or the buttons that add them to a
/// camera. A follower that is not a camera — a light trailing the player —
/// shows its section too.
pub(super) fn show(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    let camera = world.get::<Camera2D>(entity).is_some();
    let following = world.get::<CameraFollow2D>(entity).is_some();
    let shaking = world.get::<CameraShake>(entity).is_some();
    if !camera && !following && !shaking {
        return None;
    }
    ui.separator();
    let mut claim = None;
    if following {
        claim = follow::follow(ui, world, entity);
    }
    if shaking {
        claim = claim.or(shake::shake(ui, world, entity));
    }
    if camera && !(following && shaking) {
        ui.horizontal(|ui| {
            if !following && ui.button("Add follow").clicked() {
                world.insert(entity, CameraFollow2D::default());
                claim = claim.or(Some("Add camera follow"));
            }
            if !shaking && ui.button("Add shake").clicked() {
                world.insert(entity, CameraShake::default());
                claim = claim.or(Some("Add camera shake"));
            }
        });
    }
    claim
}

pub(super) fn number(
    ui: &mut Ui,
    label: &str,
    value: &mut f32,
    hint: &str,
) -> Option<&'static str> {
    ui.label(label).on_hover_text(hint);
    let response = ui.add(DragValue::new(value).speed(0.01).range(0.0..=1000.0));
    ui.end_row();
    active(&response, "Edit camera rig")
}

pub(super) fn pair(ui: &mut Ui, label: &str, value: &mut Vec2, hint: &str) -> Option<&'static str> {
    ui.label(label).on_hover_text(hint);
    let mut claim = None;
    ui.horizontal(|ui| {
        for axis in [&mut value.x, &mut value.y] {
            claim = claim.or(active(
                &ui.add(DragValue::new(axis).speed(0.01)),
                "Edit camera rig",
            ));
        }
    });
    ui.end_row();
    claim
}
