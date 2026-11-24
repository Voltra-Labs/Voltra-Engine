//! The material a sprite or tile layer is drawn with, and its uniforms.

use voltra_assets::Materials;
use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_render::material::Uniform;
use voltra_scene::SpriteMaterial;

use super::active;
use super::path_field::{self, PathEdit};

/// The material section: its path — typed or dropped — and, once it
/// resolves, a field per uniform, showing this entity's value or the
/// material's default. `slot` is the component's `material` field.
pub(super) fn show(
    ui: &mut Ui,
    slot: &mut Option<SpriteMaterial>,
    materials: Option<&mut Materials>,
) -> Option<&'static str> {
    let mut claim = None;
    ui.label(RichText::new("Material").strong())
        .on_hover_text("draws through a .material.ron's shader instead of the plain one");
    let committed = slot
        .as_ref()
        .map(|s| s.path.as_str().to_owned())
        .unwrap_or_default();
    match path_field::show(ui, "material", "materials/x.material.ron", &committed) {
        Some(PathEdit::Cleared) => {
            *slot = None;
            claim = Some("Clear material");
        }
        Some(PathEdit::Set(path)) => {
            *slot = Some(SpriteMaterial::new(path));
            claim = Some("Set material");
        }
        None => {}
    }
    let (Some(slot), Some(materials)) = (slot.as_mut(), materials) else {
        return claim;
    };
    slot.resolve(materials);
    let Some(material) = slot.handle.and_then(|h| materials.get(h)) else {
        return claim;
    };
    if let Some(error) = &material.error {
        ui.label(
            RichText::new(error)
                .color(ui.visuals().error_fg_color)
                .small(),
        );
        return claim;
    }
    let defaults = material.defaults.clone();
    egui::Grid::new("material_uniforms")
        .num_columns(2)
        .show(ui, |ui| {
            for (name, default) in &defaults {
                ui.label(name);
                let mut value = slot.uniforms.get(name).copied().unwrap_or(*default);
                if !value.same_kind(default) {
                    value = *default;
                }
                if let Some(label) = field(ui, &mut value) {
                    slot.set(name, value);
                    claim = claim.or(Some(label));
                }
                ui.end_row();
            }
        });
    if !slot.uniforms.is_empty() && ui.small_button("Defaults").clicked() {
        slot.uniforms.clear();
        claim = Some("Reset material uniforms");
    }
    claim
}

/// One uniform's widget; the label of the edit when it changed.
fn field(ui: &mut Ui, value: &mut Uniform) -> Option<&'static str> {
    const LABEL: &str = "Edit material uniform";
    match value {
        Uniform::Float(f) => active(&ui.add(DragValue::new(f).speed(0.01)), LABEL),
        Uniform::Vec2((x, y)) => {
            let mut claim = None;
            ui.horizontal(|ui| {
                for v in [x, y] {
                    claim = claim.or(active(&ui.add(DragValue::new(v).speed(0.01)), LABEL));
                }
            });
            claim
        }
        Uniform::Color((r, g, b, a)) => {
            let mut rgba = [*r, *g, *b, *a];
            let popup = ui.auto_id_with("popup");
            let response = ui.color_edit_button_rgba_unmultiplied(&mut rgba);
            [*r, *g, *b, *a] = rgba;
            (egui::Popup::is_id_open(ui.ctx(), popup) || response.changed()).then_some(LABEL)
        }
    }
}
