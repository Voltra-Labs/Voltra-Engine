//! A skinned sprite: its texture, its bones, its mesh made from the
//! texture's outline, its weights guessed and then painted.

use voltra_assets::{AssetPath, Silhouettes, TextureSizes, Textures};
use voltra_core::egui::{self, Button, DragValue, RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_render::wgpu;
use voltra_scene::keyframes::target::paths;
use voltra_scene::skeleton::skeleton_of;
use voltra_scene::skinned::{generate, pose, weights};
use voltra_scene::triangulate::signed_area2;
use voltra_scene::{Bone2D, SkinnedSprite};

use super::active;
use super::path_field::{self, PathEdit};
use crate::weight_paint::WeightPaint;

/// What the section needs besides the world.
pub(super) struct Context<'a> {
    pub entity: Entity,
    pub paint: &'a mut WeightPaint,
    pub textures: &'a mut Textures,
    pub silhouettes: Option<&'a mut Silhouettes>,
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
}

pub(super) fn show(ui: &mut Ui, world: &mut World, cx: Context<'_>) -> Option<&'static str> {
    let entity = cx.entity;
    let current = world.get::<SkinnedSprite>(entity)?.clone();
    ui.separator();
    ui.label(RichText::new("Skinned sprite").strong());
    // Read before the component is borrowed: the skeleton's bones by path,
    // and where the component's own bones lie in the mesh's space.
    let skeleton = skeleton_of(world, entity);
    let available: Vec<String> = skeleton.map_or_else(Vec::new, |root| {
        paths(world, root)
            .into_iter()
            .filter(|(path, e)| !path.is_empty() && world.get::<Bone2D>(*e).is_some())
            .map(|(path, _)| path)
            .collect()
    });
    let segments = pose::bone_segments(world, entity, &current);
    let outline = traced(&current, cx.textures, cx.silhouettes);

    let skinned = world.get_mut::<SkinnedSprite>(entity)?;
    let mut claim = None;
    let committed = skinned
        .texture
        .as_ref()
        .map(AssetPath::as_str)
        .unwrap_or("")
        .to_owned();
    ui.label("texture");
    if let Some(edit) = path_field::show(ui, "skinned_texture", "path/to/texture.png", &committed) {
        let path = match edit {
            PathEdit::Cleared => None,
            PathEdit::Set(path) => Some(path),
        };
        skinned.set_texture(path, cx.textures, cx.device, cx.queue);
        claim = Some("Set texture");
    }
    ui.horizontal(|ui| {
        ui.label("pixels / unit");
        claim = claim.or(active(
            &ui.add(DragValue::new(&mut skinned.pixels_per_unit).range(1.0..=4096.0)),
            "Edit skinned sprite",
        ));
        let popup = ui.auto_id_with("popup");
        let response = ui.color_edit_button_rgba_unmultiplied(&mut skinned.color);
        if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
            claim = claim.or(Some("Edit skinned sprite"));
        }
    });

    ui.label(format!("bones: {}", skinned.bones.join(", ")));
    if skeleton.is_none() {
        ui.label(RichText::new("under no skeleton: it draws rigid").weak());
    } else if ui
        .button("Use the skeleton's bones")
        .on_hover_text("every bone under the skeleton, by path")
        .clicked()
    {
        skinned.bones = available.clone();
        claim = Some("Set skinned bones");
    }

    ui.label(
        RichText::new(format!(
            "{} vertices, {} triangles",
            skinned.mesh.vertices.len(),
            skinned.mesh.triangles.len()
        ))
        .weak(),
    );
    ui.horizontal(|ui| {
        let generate_clicked = ui
            .add_enabled(outline.is_some(), Button::new("Generate mesh"))
            .on_hover_text("from the texture's opaque outline, cut on a grid; weights guessed")
            .clicked();
        if let (true, Some((outline, size))) = (generate_clicked, &outline) {
            let ppu = skinned.pixels_per_unit.max(1.0);
            let units = generate::outline_in_units(outline, *size, ppu);
            let extent = *size / ppu;
            let cell = (extent.min_element() / 4.0).max(extent.max_element() / 48.0);
            skinned.mesh = generate::from_outline(&units, cell, |p| generate::uv_at(p, *size, ppu));
            weights::automatic(&mut skinned.mesh, &segments);
            claim = Some("Generate skinned mesh");
        }
        if ui
            .add_enabled(!skinned.bones.is_empty(), Button::new("Auto weights"))
            .on_hover_text("each vertex held by its nearest bones")
            .clicked()
        {
            weights::automatic(&mut skinned.mesh, &segments);
            claim = Some("Auto weights");
        }
    });

    let painting = cx.paint.is_painting(entity);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !skinned.bones.is_empty(),
                Button::selectable(painting, "Paint weights"),
            )
            .on_hover_text("drag in the viewport to add the bone's weight; Shift takes it away")
            .clicked()
        {
            cx.paint.set_painting((!painting).then_some(entity));
        }
        let bone = cx.paint.bone as usize;
        egui::ComboBox::from_id_salt("weight_bone")
            .selected_text(skinned.bones.get(bone).map_or("—", String::as_str))
            .show_ui(ui, |ui| {
                for (i, name) in skinned.bones.iter().enumerate() {
                    ui.selectable_value(&mut cx.paint.bone, i as u32, name);
                }
            });
    });
    if painting {
        ui.horizontal(|ui| {
            ui.label("brush");
            ui.add(
                DragValue::new(&mut cx.paint.radius)
                    .speed(0.01)
                    .range(0.01..=10.0),
            );
            ui.label("strength");
            ui.add(
                DragValue::new(&mut cx.paint.strength)
                    .speed(0.05)
                    .range(0.05..=20.0),
            );
        });
    }
    claim
}

/// The largest outline of the component's whole texture and the texture's
/// size in texels, traced now if it has not been — or `None` without a
/// loaded texture or anything opaque in it.
fn traced(
    skinned: &SkinnedSprite,
    textures: &Textures,
    silhouettes: Option<&mut Silhouettes>,
) -> Option<(Vec<Vec2>, Vec2)> {
    let (path, handle) = (skinned.texture.as_ref()?, skinned.texture_handle?);
    let size = textures.size(handle)?;
    let silhouettes = silhouettes?;
    let rect = [0, 0, size.x, size.y];
    let threshold = voltra_assets::silhouette::ALPHA_THRESHOLD;
    silhouettes.ensure(path, rect, threshold);
    let outline = silhouettes
        .get(path, rect, threshold)?
        .iter()
        .max_by(|a, b| signed_area2(a).abs().total_cmp(&signed_area2(b).abs()))?
        .clone();
    Some((outline, size.as_vec2()))
}
