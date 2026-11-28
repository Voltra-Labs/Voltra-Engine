//! A point light's colour, strength, reach, shadows, cone, layers and
//! cookie.

use voltra_assets::{AssetPath, Textures};
use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_render::lights::MAX_SHADOW_SOFTNESS;
use voltra_render::wgpu;
use voltra_scene::PointLight2D;

use super::active;
use super::path_field::{self, PathEdit};

pub(super) fn show(
    ui: &mut Ui,
    light: &mut PointLight2D,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<&'static str> {
    ui.label(RichText::new("Point light").strong())
        .on_hover_text("lights only a frame whose camera is lit");
    let mut claim = None;
    egui::Grid::new("point_light")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("colour");
            let popup = ui.auto_id_with("popup");
            let response = crate::panels::colour_edit::rgb(ui, &mut light.color);
            if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
                claim = claim.or(Some("Edit light"));
            }
            ui.end_row();

            for (label, value, range) in [
                ("intensity", &mut light.intensity, 0.0..=16.0),
                ("radius", &mut light.radius, 0.0..=1000.0),
                ("falloff", &mut light.falloff, 0.05..=16.0),
            ] {
                ui.label(label);
                claim = claim.or(active(
                    &ui.add(DragValue::new(value).speed(0.01).range(range)),
                    "Edit light",
                ));
                ui.end_row();
            }

            ui.label("shadows")
                .on_hover_text("blocked by shadow casters; off, it shines through everything");
            if optional(ui, "light_shadows", &mut light.shadows) {
                claim = claim.or(Some("Toggle light shadows"));
            }
            ui.end_row();

            if let Some(shadows) = &mut light.shadows {
                ui.label("strength")
                    .on_hover_text("how much of the light a shadow takes away");
                claim = claim.or(active(
                    &ui.add(
                        DragValue::new(&mut shadows.strength)
                            .speed(0.01)
                            .range(0.0..=1.0)
                            .clamp_existing_to_range(false),
                    ),
                    "Edit light",
                ));
                ui.end_row();

                // Stored in radians, shown in degrees, like a rotation.
                ui.label("softness")
                    .on_hover_text("how far either side of a shadow's edge it fades");
                let mut degrees = shadows.softness.to_degrees();
                let response = ui.add(
                    DragValue::new(&mut degrees)
                        .speed(0.05)
                        .range(0.0..=MAX_SHADOW_SOFTNESS.to_degrees())
                        .clamp_existing_to_range(false)
                        .suffix("°"),
                );
                if response.changed() {
                    shadows.softness = degrees.to_radians();
                }
                claim = claim.or(active(&response, "Edit light"));
                ui.end_row();
            }

            ui.label("height").on_hover_text(
                "how far above the scene it sits, for normal maps; off ignores them",
            );
            if optional_height(ui, &mut light.height) {
                claim = claim.or(Some("Toggle light height"));
            }
            ui.end_row();
            if let Some(height) = &mut light.height {
                // A height written as zero or less by hand is no height: say
                // so rather than show a ticked box that does nothing.
                if !(height.is_finite() && *height > 0.0) {
                    ui.label(RichText::new("ignored").weak())
                        .on_hover_text("a height of zero or less is no height");
                } else {
                    ui.label("");
                }
                claim = claim.or(active(
                    &ui.add(
                        DragValue::new(height)
                            .speed(0.01)
                            .range(0.01..=100.0)
                            .clamp_existing_to_range(false),
                    ),
                    "Edit light",
                ));
                ui.end_row();
            }

            ui.label("spot")
                .on_hover_text("shines through a cone along the entity's +x; turn it to aim");
            if optional(ui, "light_cone", &mut light.cone) {
                claim = claim.or(Some("Toggle spot light"));
            }
            ui.end_row();

            if let Some(cone) = &mut light.cone {
                ui.label("angle")
                    .on_hover_text("the whole angle across the cone");
                let mut degrees = cone.angle.to_degrees();
                let response = ui.add(
                    DragValue::new(&mut degrees)
                        .speed(0.5)
                        .range(1.0..=360.0)
                        .clamp_existing_to_range(false)
                        .suffix("°"),
                );
                if response.changed() {
                    cone.angle = degrees.to_radians();
                }
                claim = claim.or(active(&response, "Edit light"));
                ui.end_row();

                ui.label("edge")
                    .on_hover_text("how much of the cone, from its edge inward, fades");
                claim = claim.or(active(
                    &ui.add(
                        DragValue::new(&mut cone.softness)
                            .speed(0.01)
                            .range(0.0..=1.0)
                            .clamp_existing_to_range(false),
                    ),
                    "Edit light",
                ));
                ui.end_row();
            }
        });
    claim = claim.or(super::light_layers::show(
        ui,
        "light_layers",
        "lights these layers",
        "lights only what is on one of these",
        &mut light.layers,
    ));
    claim.or(cookie(ui, light, textures, device, queue))
}

/// The cookie's path: an image laid over the light that tints and shapes it.
fn cookie(
    ui: &mut Ui,
    light: &mut PointLight2D,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<&'static str> {
    ui.label("cookie").on_hover_text(
        "an image over the light's reach that tints and shapes it; turns with the entity",
    );
    let committed = light
        .cookie
        .as_ref()
        .map(AssetPath::as_str)
        .unwrap_or("")
        .to_owned();
    let path = match path_field::show(ui, "light_cookie", "path/to/cookie.png", &committed)? {
        PathEdit::Cleared => None,
        PathEdit::Set(path) => Some(path),
    };
    light.set_cookie(path, textures, device, queue);
    Some("Set light cookie")
}

/// The height checkbox: [`optional`] for a plain number, which has no
/// useful default of zero — a light at the scene's own level rakes so
/// low that every bump is black on one side.
fn optional_height(ui: &mut Ui, height: &mut Option<f32>) -> bool {
    let mut wrapped = height.map(Height);
    let toggled = optional(ui, "light_height", &mut wrapped);
    *height = wrapped.map(|h| h.0);
    toggled
}

/// A light height with a default worth starting from: one unit up.
#[derive(Clone, Copy)]
struct Height(f32);

impl Default for Height {
    fn default() -> Self {
        Self(1.0)
    }
}

/// A checkbox for an optional part of the light. Returns whether it was
/// toggled — one click, so that is the whole edit. What was tuned is kept
/// in egui's memory under `key` while it is off (the inspector's body is
/// salted by entity, so per light) and comes back with it.
pub(super) fn optional<T>(ui: &mut Ui, key: &str, value: &mut Option<T>) -> bool
where
    T: Clone + Default + Send + Sync + 'static,
{
    let mut on = value.is_some();
    if !ui.checkbox(&mut on, "").changed() {
        return false;
    }
    let kept = ui.make_persistent_id(key);
    if let Some(off) = value.take() {
        ui.data_mut(|data| data.insert_temp(kept, off));
    }
    if on {
        *value = Some(ui.data(|data| data.get_temp::<T>(kept)).unwrap_or_default());
    }
    true
}
