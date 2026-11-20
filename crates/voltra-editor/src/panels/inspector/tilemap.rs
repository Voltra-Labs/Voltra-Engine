//! A tilemap layer's look, its tileset, and what it holds.
//!
//! Cells are painted in the viewport, the way Godot's and Unity's tile
//! palettes work; what this section edits is the layer as a whole, plus the
//! palette that picks the brush.

mod palette;

pub(super) use palette::Palette;

use voltra_assets::{AssetPath, Atlases, Textures};
use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_render::wgpu;
use voltra_scene::Tilemap;

use super::active;
use super::path_field::{self, PathEdit};

/// The smallest cell side the field accepts. A zero-sized cell draws nothing
/// and puts every tile on one point; a negative one mirrors the layer, which
/// is the transform's job.
const MIN_TILE: f32 = 0.01;

pub(super) fn show(
    ui: &mut Ui,
    map: &mut Tilemap,
    textures: &mut Textures,
    atlases: &mut Atlases,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    palette: Palette<'_>,
) -> Option<&'static str> {
    ui.label(RichText::new("Tilemap").strong());

    let mut claim = None;
    egui::Grid::new("tilemap").num_columns(2).show(ui, |ui| {
        ui.label("colour");
        // See the sprite section: the popup's id has to be read before the
        // button, or a colour pick is one entry per frame.
        let popup = ui.auto_id_with("popup");
        let response = ui.color_edit_button_rgba_unmultiplied(&mut map.color);
        if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
            claim = claim.or(Some("Set colour"));
        }
        ui.end_row();

        ui.label("sort order");
        claim = claim.or(active(
            &ui.add(DragValue::new(&mut map.sort_order)),
            "Set sort order",
        ));
        ui.end_row();

        ui.label("tile size");
        ui.horizontal(|ui| {
            for axis in [&mut map.tile_size.x, &mut map.tile_size.y] {
                let field = DragValue::new(axis).speed(0.01).range(MIN_TILE..=f32::MAX);
                claim = claim.or(active(&ui.add(field), "Set tile size"));
            }
        });
        ui.end_row();

        ui.label("collision");
        if ui
            .checkbox(&mut map.collision, "solid")
            .on_hover_text("Every tile of this layer is solid; boxes are generated from the tiles")
            .changed()
        {
            claim = claim.or(Some("Set collision"));
        }
        ui.end_row();

        ui.label("tiles");
        let held = match map.bounds() {
            Some((min, max)) => format!(
                "{} in ({}, {})..({}, {})",
                map.len(),
                min.x,
                min.y,
                max.x,
                max.y
            ),
            None => "none".to_owned(),
        };
        ui.label(RichText::new(held).weak());
        ui.end_row();
    });

    ui.separator();
    claim = claim.or(texture(ui, map, textures, device, queue));
    claim = claim.or(atlas(ui, map, atlases));

    if let Some(set) = map.atlas_handle.and_then(|handle| atlases.try_get(handle)) {
        ui.separator();
        palette::show(ui, set, palette);
    }
    claim
}

fn texture(
    ui: &mut Ui,
    map: &mut Tilemap,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<&'static str> {
    ui.label(RichText::new("Sheet").strong());
    let committed = shown(map.texture.as_ref());
    let path = match path_field::show(ui, "tilemap_texture", "path/to/tiles.png", &committed)? {
        PathEdit::Cleared => None,
        PathEdit::Set(path) => Some(path),
    };
    map.set_texture(path, textures, device, queue);
    Some("Set texture")
}

/// The tileset: which slicing the cells' frame numbers index, and how many
/// frames it has — the highest frame a cell can usefully name.
fn atlas(ui: &mut Ui, map: &mut Tilemap, atlases: &mut Atlases) -> Option<&'static str> {
    ui.label(RichText::new("Tileset").strong());
    let committed = shown(map.atlas.as_ref());
    let claim = match path_field::show(ui, "tilemap_atlas", "path/to/tiles.atlas.ron", &committed) {
        Some(PathEdit::Cleared) => {
            map.set_atlas(None, atlases);
            Some("Set tileset")
        }
        Some(PathEdit::Set(path)) => {
            map.set_atlas(Some(path), atlases);
            Some("Set tileset")
        }
        None => None,
    };
    // Read after the assignment, so a set dropped this frame shows its own
    // count rather than the previous one's.
    if map.atlas.is_some() {
        let frames = map
            .atlas_handle
            .and_then(|handle| atlases.try_get(handle))
            .map_or(0, |atlas| atlas.len());
        ui.label(RichText::new(format!("{frames} tiles in the set")).weak());
    }
    claim
}

/// A path as its text box spells it.
fn shown(path: Option<&AssetPath>) -> String {
    path.map(AssetPath::as_str).unwrap_or("").to_owned()
}
