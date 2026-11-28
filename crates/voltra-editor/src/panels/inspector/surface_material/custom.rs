//! A custom surface's own part of its file: the shader, whether it is lit,
//! its uniforms and its textures — each a field, added and removed here.

use voltra_assets::CustomTextureFile;
use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_render::material::Uniform;
use voltra_render::render3d::{SurfaceLighting, TextureSpace, CUSTOM_TEXTURES};

use super::super::path_field::{self, PathEdit};
use crate::surface_doc::SurfaceDoc;

/// The shader's path and, when there is one, its lighting, uniforms and
/// textures. Edits land in `doc`'s draft, written when they settle.
pub(super) fn show(ui: &mut Ui, doc: &mut SurfaceDoc) {
    ui.separator();
    ui.label(RichText::new("custom surface").weak())
        .on_hover_text(
            "a WGSL file defining fn surface(in: SurfaceInput) -> Surface; \
             it may start from standard_surface(in)",
        );
    egui::Grid::new("surface_custom")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("shader");
            let committed = doc.shader_mut().clone();
            match path_field::show(ui, "surface_shader", "materials/x.wgsl", &committed) {
                Some(PathEdit::Set(path)) => *doc.shader_mut() = path.as_str().to_owned(),
                Some(PathEdit::Cleared) => doc.shader_mut().clear(),
                None => {}
            }
            ui.end_row();
            if doc.shader_mut().trim().is_empty() {
                return;
            }
            ui.label("lighting");
            let lighting = doc.lighting_mut();
            egui::ComboBox::from_id_salt("surface_lighting")
                .selected_text(format!("{lighting:?}"))
                .show_ui(ui, |ui| {
                    for value in [SurfaceLighting::Lit, SurfaceLighting::Unlit] {
                        ui.selectable_value(lighting, value, format!("{value:?}"));
                    }
                });
            ui.end_row();
        });
    if doc.shader_mut().trim().is_empty() {
        return;
    }
    uniforms(ui, doc);
    textures(ui, doc);
}

/// A field per uniform, a button to remove each, and a row to add one.
fn uniforms(ui: &mut Ui, doc: &mut SurfaceDoc) {
    ui.label(RichText::new("uniforms").weak())
        .on_hover_text("read in the shader as material.<name>");
    let mut removed = None;
    egui::Grid::new("surface_uniforms")
        .num_columns(3)
        .show(ui, |ui| {
            for (name, value) in doc.uniforms_mut().iter_mut() {
                ui.push_id(name.as_str(), |ui| {
                    ui.label(name);
                    uniform(ui, value);
                    if ui.small_button("×").on_hover_text("remove").clicked() {
                        removed = Some(name.clone());
                    }
                });
                ui.end_row();
            }
        });
    if let Some(name) = removed {
        doc.uniforms_mut().remove(&name);
    }
    const KINDS: [(&str, Uniform); 3] = [
        ("Float", Uniform::Float(0.0)),
        ("Vec2", Uniform::Vec2((0.0, 0.0))),
        ("Color", Uniform::Color((1.0, 1.0, 1.0, 1.0))),
    ];
    let taken: Vec<String> = doc.uniforms_mut().keys().cloned().collect();
    if let Some((name, kind)) = add_row(ui, "surface_add_uniform", &taken, &KINDS, true) {
        doc.uniforms_mut().insert(name, kind);
    }
}

/// One uniform's widget, writing `value` only when the person changes it:
/// a colour picker converts through HSV every frame it is drawn, and
/// writing that back would rewrite the file with a drifted number just by
/// showing it. A colour past 1 — light, not paint — gets four numbers,
/// since the picker would clamp it.
fn uniform(ui: &mut Ui, value: &mut Uniform) {
    match value {
        Uniform::Float(f) => {
            ui.add(DragValue::new(f).speed(0.01));
        }
        Uniform::Vec2((x, y)) => {
            ui.horizontal(|ui| {
                ui.add(DragValue::new(x).speed(0.01));
                ui.add(DragValue::new(y).speed(0.01));
            });
        }
        Uniform::Color((r, g, b, a)) => {
            let mut rgba = [*r, *g, *b, *a];
            if rgba.iter().all(|c| (0.0..=1.0).contains(c)) {
                if crate::panels::colour_edit::rgba(ui, &mut rgba).changed() {
                    [*r, *g, *b, *a] = rgba;
                }
            } else {
                ui.horizontal(|ui| {
                    for c in [r, g, b, a] {
                        ui.add(DragValue::new(c).speed(0.01));
                    }
                });
            }
        }
    }
}

/// A row per texture — its path, colour or data, remove — and a row to add
/// one, up to the four a surface may sample.
fn textures(ui: &mut Ui, doc: &mut SurfaceDoc) {
    ui.label(RichText::new("textures").weak()).on_hover_text(
        "read in the shader by name: textureSample(<name>, surface_sampler, uv); \
         colour is decoded from sRGB, data read as stored",
    );
    let mut removed = None;
    egui::Grid::new("surface_textures")
        .num_columns(3)
        .show(ui, |ui| {
            for (name, texture) in doc.textures_mut().iter_mut() {
                ui.push_id(name.as_str(), |ui| {
                    ui.label(name);
                    ui.horizontal(|ui| {
                        let committed = texture.path().to_owned();
                        match path_field::show(ui, "texture", "textures/x.png", &committed) {
                            Some(PathEdit::Set(path)) => {
                                *texture.path_mut() = path.as_str().to_owned()
                            }
                            Some(PathEdit::Cleared) => texture.path_mut().clear(),
                            None => {}
                        }
                        let mut space = texture.space();
                        egui::ComboBox::from_id_salt("space")
                            .selected_text(format!("{space:?}"))
                            .width(60.0)
                            .show_ui(ui, |ui| {
                                for value in [TextureSpace::Color, TextureSpace::Data] {
                                    ui.selectable_value(&mut space, value, format!("{value:?}"));
                                }
                            });
                        if space != texture.space() {
                            *texture = texture.with_space(space);
                        }
                    });
                    if ui.small_button("×").on_hover_text("remove").clicked() {
                        removed = Some(name.clone());
                    }
                });
                ui.end_row();
            }
        });
    if let Some(name) = removed {
        doc.textures_mut().remove(&name);
    }
    let taken: Vec<String> = doc.textures_mut().keys().cloned().collect();
    let room = taken.len() < CUSTOM_TEXTURES;
    const KINDS: [(&str, TextureSpace); 2] =
        [("Color", TextureSpace::Color), ("Data", TextureSpace::Data)];
    if let Some((name, space)) = add_row(ui, "surface_add_texture", &taken, &KINDS, room) {
        let texture = match space {
            TextureSpace::Color => CustomTextureFile::Color(String::new()),
            TextureSpace::Data => CustomTextureFile::Data(String::new()),
        };
        doc.textures_mut().insert(name, texture);
    }
    if !room {
        ui.label(
            RichText::new(format!("{CUSTOM_TEXTURES} textures: a surface's most"))
                .weak()
                .small(),
        );
    }
}

/// A name box, a kind and an Add button, enabled while `room` and the name
/// is a WGSL identifier not in `taken`: the name and kind added, once.
fn add_row<T: Copy + Send + Sync + 'static>(
    ui: &mut Ui,
    salt: &'static str,
    taken: &[String],
    kinds: &[(&'static str, T)],
    room: bool,
) -> Option<(String, T)> {
    let id = ui.id().with(salt);
    let (mut name, mut kind) = ui
        .ctx()
        .data_mut(|d| d.get_temp::<(String, usize)>(id))
        .unwrap_or_default();
    let kind_index = kind.min(kinds.len().saturating_sub(1));
    let mut added = None;
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut name)
                .hint_text("name")
                .desired_width(90.0),
        );
        egui::ComboBox::from_id_salt(id.with("kind"))
            .selected_text(kinds[kind_index].0)
            .width(60.0)
            .show_ui(ui, |ui| {
                for (i, (label, _)) in kinds.iter().enumerate() {
                    ui.selectable_value(&mut kind, i, *label);
                }
            });
        let valid = room && is_name(&name) && !taken.contains(&name);
        if ui.add_enabled(valid, egui::Button::new("Add")).clicked() {
            added = Some((name.clone(), kinds[kind.min(kinds.len() - 1)].1));
            name.clear();
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(id, (name, kind)));
    added
}

/// Whether `name` is one WGSL takes: a letter or `_`, then letters, digits
/// and `_`, not starting `__`. A keyword gets past this and is refused by
/// the shader's check, with naga's message.
fn is_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.starts_with("__")
}
