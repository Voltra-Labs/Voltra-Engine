//! The sprite's colour, draw order and texture.

mod atlas;

use voltra_assets::{AssetPath, Atlases, Textures};
use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_core::ui::TextureId;
use voltra_render::wgpu;
use voltra_scene::Sprite;

use super::active;
use super::path_field::{self, PathEdit};

pub(super) fn show(
    ui: &mut Ui,
    sprite: &mut Sprite,
    textures: &mut Textures,
    atlases: &mut Atlases,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    preview: Option<TextureId>,
) -> Option<&'static str> {
    ui.label(RichText::new("Sprite").strong());

    let mut claim = None;
    egui::Grid::new("sprite").num_columns(2).show(ui, |ui| {
        ui.label("colour");
        // The colour is edited inside a popup, so the button itself is never
        // dragged or focused while the value moves and [`active`] would report
        // nothing — one entry per frame of the pick. The popup's id is
        // `ui.auto_id_with("popup")` taken before the button is allocated
        // (egui-0.35 `widgets/color_picker.rs:519`), and `auto_id_with` does
        // not advance the counter, so reading it here gives the same id the
        // widget will derive a line later.
        let popup = ui.auto_id_with("popup");
        let response = ui.color_edit_button_rgba_unmultiplied(&mut sprite.color);
        if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
            claim = claim.or(Some("Set colour"));
        }
        ui.end_row();

        ui.label("sort order");
        claim = claim.or(active(
            &ui.add(DragValue::new(&mut sprite.sort_order)),
            "Set sort order",
        ));
        ui.end_row();
    });

    ui.separator();
    claim = claim.or(texture_ui(ui, sprite, textures, device, queue));
    ui.separator();
    claim.or(atlas::show(ui, sprite, atlases, preview))
}

/// The texture path editor.
fn texture_ui(
    ui: &mut Ui,
    sprite: &mut Sprite,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<&'static str> {
    ui.label(RichText::new("Texture").strong());

    let committed = committed_path(sprite);
    let path = match path_field::show(ui, "texture_path", "path/to/texture.png", &committed)? {
        PathEdit::Cleared => None,
        PathEdit::Set(path) => Some(path),
    };
    sprite.set_texture(path, textures, device, queue);
    Some("Set texture")
}

/// The path a sprite is actually showing, as the text box spells it.
fn committed_path(sprite: &Sprite) -> String {
    sprite
        .texture
        .as_ref()
        .map(AssetPath::as_str)
        .unwrap_or("")
        .to_owned()
}
