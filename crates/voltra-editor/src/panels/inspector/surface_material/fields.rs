//! A surface material's standard block as fields, one group per block:
//! base colour, metallic-roughness, normal, occlusion, emissive, alpha and
//! the UV transform.

use voltra_assets::StandardFile;
use voltra_core::egui::{self, DragValue, Slider, Ui};
use voltra_render::render3d::{AlphaMode, UvTransform};

use super::super::path_field::{self, PathEdit};

/// Every field of `standard`. Colours are linear, picked in sRGB; a
/// texture is a path typed or dropped from the asset browser.
pub(super) fn show(ui: &mut Ui, standard: &mut StandardFile) {
    egui::Grid::new("surface_material")
        .num_columns(2)
        .show(ui, |ui| {
            heading(ui, "base");
            ui.label("colour");
            ui.color_edit_button_rgba_unmultiplied(&mut standard.base_color);
            ui.end_row();
            texture(
                ui,
                "base_texture",
                "colour (sRGB) and alpha",
                &mut standard.base_color_texture,
            );

            heading(ui, "metallic-roughness");
            ui.label("metallic");
            ui.add(Slider::new(&mut standard.metallic, 0.0..=1.0));
            ui.end_row();
            ui.label("roughness");
            ui.add(Slider::new(&mut standard.roughness, 0.0..=1.0));
            ui.end_row();
            texture(
                ui,
                "metallic_roughness_texture",
                "data: G roughness, B metallic",
                &mut standard.metallic_roughness_texture,
            );

            heading(ui, "normal");
            texture(
                ui,
                "normal_texture",
                "data: a tangent-space normal map",
                &mut standard.normal_texture,
            );
            ui.label("scale");
            ui.add(DragValue::new(&mut standard.normal_scale).speed(0.01));
            ui.end_row();

            heading(ui, "occlusion");
            texture(
                ui,
                "occlusion_texture",
                "data: R occlusion",
                &mut standard.occlusion_texture,
            );
            ui.label("strength");
            ui.add(Slider::new(&mut standard.occlusion_strength, 0.0..=1.0));
            ui.end_row();

            heading(ui, "emissive");
            ui.label("colour");
            ui.color_edit_button_rgb(&mut standard.emissive);
            ui.end_row();
            ui.label("strength")
                .on_hover_text("in the light units' energy");
            ui.add(
                DragValue::new(&mut standard.emissive_strength)
                    .speed(0.01)
                    .range(0.0..=f32::MAX),
            );
            ui.end_row();
            texture(
                ui,
                "emissive_texture",
                "colour (sRGB)",
                &mut standard.emissive_texture,
            );

            heading(ui, "alpha");
            alpha(ui, &mut standard.alpha);
            ui.label("double-sided");
            ui.checkbox(&mut standard.double_sided, "")
                .on_hover_text("both faces drawn, the back lit as the front turned round");
            ui.end_row();
            ui.label("unlit");
            ui.checkbox(&mut standard.unlit, "")
                .on_hover_text("the base colour drawn as it is, no light");
            ui.end_row();

            heading(ui, "UV transform");
            uv(ui, &mut standard.uv_transform);
        });
}

fn heading(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).weak());
    ui.end_row();
}

/// A texture slot: a path field, dropped onto from the asset browser.
fn texture(ui: &mut Ui, salt: &'static str, hover: &str, slot: &mut String) {
    ui.label("texture").on_hover_text(hover);
    match path_field::show(ui, salt, "textures/x.png", slot) {
        Some(PathEdit::Set(path)) => *slot = path.as_str().to_owned(),
        Some(PathEdit::Cleared) => slot.clear(),
        None => {}
    }
    ui.end_row();
}

/// The alpha mode and a mask's cutoff.
fn alpha(ui: &mut Ui, mode: &mut AlphaMode) {
    const NAMES: [&str; 3] = ["Opaque", "Mask", "Blend"];
    let shown = match mode {
        AlphaMode::Opaque => NAMES[0],
        AlphaMode::Mask(_) => NAMES[1],
        AlphaMode::Blend => NAMES[2],
    };
    ui.label("mode");
    egui::ComboBox::from_id_salt("surface_alpha")
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for (name, value) in NAMES.iter().zip([
                AlphaMode::Opaque,
                AlphaMode::Mask(AlphaMode::DEFAULT_CUTOFF),
                AlphaMode::Blend,
            ]) {
                if ui.selectable_label(shown == *name, *name).clicked() && shown != *name {
                    *mode = value;
                }
            }
        });
    ui.end_row();
    if let AlphaMode::Mask(cutoff) = mode {
        ui.label("cutoff");
        ui.add(Slider::new(cutoff, 0.0..=1.0));
        ui.end_row();
    }
}

/// Offset, scale, and the rotation in degrees.
fn uv(ui: &mut Ui, transform: &mut UvTransform) {
    for (name, pair) in [
        ("offset", &mut transform.offset),
        ("scale", &mut transform.scale),
    ] {
        ui.label(name);
        ui.horizontal(|ui| {
            for v in pair.iter_mut() {
                ui.add(DragValue::new(v).speed(0.01));
            }
        });
        ui.end_row();
    }
    ui.label("rotation").on_hover_text("counter-clockwise");
    ui.drag_angle(&mut transform.rotation);
    ui.end_row();
}
