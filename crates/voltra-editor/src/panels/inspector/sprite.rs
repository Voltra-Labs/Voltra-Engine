//! The sprite's colour, draw order and texture.

use voltra_assets::{AssetPath, Textures};
use voltra_core::egui::{self, DragValue, Frame, RichText, Stroke, TextEdit, Ui};
use voltra_render::wgpu;
use voltra_scene::Sprite;

use super::active;

pub(super) fn show(
    ui: &mut Ui,
    sprite: &mut Sprite,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
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
    claim.or(texture_ui(ui, sprite, textures, device, queue))
}

/// The texture path editor: a `TextEdit` plus a `Clear` button.
///
/// The typed text lives in egui's own per-id storage, not a field on
/// `Editor` — the id is salted by `entity` through the caller's `push_id`,
/// so switching the selection lands on a fresh slot instead of showing the
/// previous entity's half-typed path. Seeded once from `sprite.texture` and
/// left alone after that: re-cloning the path into the buffer every frame
/// would erase whatever the user just typed the moment the buffer is read
/// back before a commit.
fn texture_ui(
    ui: &mut Ui,
    sprite: &mut Sprite,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<&'static str> {
    ui.label(RichText::new("Texture").strong());

    let buffer_id = ui.id().with("texture_path");
    // Owned rather than borrowed from `sprite.texture`: the comparison below
    // runs inside a closure that also calls `sprite.set_texture`, and a
    // `&str` still borrowing `sprite` at that point would fight that
    // mutation for no reason — the committed path does not change shape
    // once read.
    let committed = committed_path(sprite);
    let mut buffer = ui.ctx().data_mut(|data| {
        data.get_temp_mut_or_insert_with::<String>(buffer_id, || committed.clone())
            .clone()
    });

    let mut claim = None;
    // The field is a drop target as well as a text box, which is how the same
    // assignment is made in Unity, Unreal and Godot: a path is typed once and
    // dragged from the browser every time after that. The frame gains a border
    // while a payload is in the air so the target is visible before the release
    // rather than discovered by trying it.
    let armed = egui::DragAndDrop::has_payload_of_type::<AssetPath>(ui.ctx());
    let zone = if armed {
        Frame::default().stroke(Stroke::new(1.5, ui.visuals().selection.bg_fill))
    } else {
        Frame::default()
    };

    let (_, dropped) = ui.dnd_drop_zone::<AssetPath, _>(zone, |ui| {
        ui.horizontal(|ui| {
            let response =
                ui.add(TextEdit::singleline(&mut buffer).hint_text("path/to/texture.png"));
            let clear_clicked = ui.button("Clear").clicked();

            // Clicking `Clear` also moves focus off the `TextEdit`, so both
            // conditions can be true on the same frame; `else if` makes Clear
            // win rather than letting the stale buffer commit and then
            // immediately get overwritten, which loaded a texture just to
            // discard it.
            //
            // `lost_focus` alone also covers Enter: a singleline `TextEdit`
            // surrenders focus on it. Guarded on a real change so clicking in
            // and out without editing does not re-run `set_texture` for no
            // reason.
            if clear_clicked {
                claim = Some("Set texture");
                sprite.set_texture(None, textures, device, queue);
                buffer.clear();
            } else if response.lost_focus() && buffer != committed {
                claim = Some("Set texture");
                match AssetPath::new(&buffer) {
                    Ok(path) => sprite.set_texture(Some(path), textures, device, queue),
                    Err(e) => log::error!("invalid texture path {buffer:?}: {e}"),
                }
                // Resyncs the box: an accepted edit shows the normalised path,
                // a rejected one reverts rather than leaving bad text sitting
                // there looking committed.
                buffer = committed_path(sprite);
            }
        });
    });

    // After the box, so a path dropped onto a field being typed into wins: the
    // drop is the more deliberate of the two gestures, and the text it replaces
    // was never committed.
    if let Some(path) = dropped {
        claim = Some("Set texture");
        sprite.set_texture(Some((*path).clone()), textures, device, queue);
        buffer = committed_path(sprite);
    }

    ui.ctx()
        .data_mut(|data| data.insert_temp(buffer_id, buffer));

    claim
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
