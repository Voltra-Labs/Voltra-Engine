//! A point light's colour, strength, reach and shadows.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_render::lights::MAX_SHADOW_SOFTNESS;
use voltra_scene::{LightShadows, PointLight2D};

use super::active;

pub(super) fn show(ui: &mut Ui, light: &mut PointLight2D) -> Option<&'static str> {
    ui.label(RichText::new("Point light").strong())
        .on_hover_text("lights only a frame whose camera is lit");
    let mut claim = None;
    egui::Grid::new("point_light")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("colour");
            let popup = ui.auto_id_with("popup");
            let response = ui.color_edit_button_rgb(&mut light.color);
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
            let mut casts = light.shadows.is_some();
            // A checkbox is one click, so `changed` is the whole edit. What
            // was tuned is kept in egui's memory while it is off — the body
            // is salted by entity, so per light — and comes back with it.
            let kept = ui.make_persistent_id("light_shadows");
            if ui.checkbox(&mut casts, "").changed() {
                if let Some(off) = light.shadows.take() {
                    ui.data_mut(|data| data.insert_temp(kept, off));
                }
                if casts {
                    light.shadows = Some(
                        ui.data(|data| data.get_temp::<LightShadows>(kept))
                            .unwrap_or_default(),
                    );
                }
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
        });
    claim
}
