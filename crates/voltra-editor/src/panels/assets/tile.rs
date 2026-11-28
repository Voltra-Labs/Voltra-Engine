//! The tiles the asset browser lays out: one per entry, drawn by kind.

use voltra_assets::browse::{Entry, EntryKind};
use voltra_core::egui::{self, Color32, CursorIcon, Rect, Sense, Ui, Vec2};
use voltra_core::UiFrame;

use crate::drag;
use crate::editor::Editor;

mod scene;

/// Side of a tile's picture, in points.
const TILE: f32 = 64.0;

/// Height the name under a tile gets.
const LABEL: f32 = 16.0;

/// One directory or file.
pub(super) fn tile(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>, entry: &Entry) {
    let (rect, response) = allocate(ui);
    // The full path, because the name under a tile is clipped and two
    // directories deep in the tree can share one.
    let response = response.on_hover_text(entry.path.as_str());

    match entry.kind {
        EntryKind::Directory => {
            paint_block(ui, rect, ui.visuals().widgets.inactive.bg_fill);
            if response.clicked() {
                editor.assets.go(Some(entry.path.clone()));
            }
            // The texture packer, on the folder of loose sprites it packs.
            response.context_menu(|ui| {
                for (label, trim) in [
                    ("Pack into atlas", false),
                    ("Pack into atlas, trimmed", true),
                ] {
                    if ui.button(label).clicked() {
                        match crate::pack_atlas::pack(frame, &entry.path, trim) {
                            Ok(_) => editor.assets.refresh(),
                            Err(e) => log::error!("packing {}: {e}", entry.path.as_str()),
                        }
                        ui.close();
                    }
                }
            });
        }
        EntryKind::Texture => {
            texture_tile(ui, frame, entry, &response);
            response.context_menu(|ui| {
                if ui.button("Slice in Atlas Editor").clicked() {
                    open_in_atlas_editor(editor, frame, entry);
                    ui.close();
                }
            });
        }
        EntryKind::Atlas => {
            atlas_tile(ui, frame, entry, &response);
            if response.double_clicked() {
                open_in_atlas_editor(editor, frame, entry);
            }
            response.context_menu(|ui| {
                if ui.button("Edit in Atlas Editor").clicked() {
                    open_in_atlas_editor(editor, frame, entry);
                    ui.close();
                }
            });
        }
        EntryKind::Clip => clip_tile(ui, entry, &response),
        EntryKind::Font => font_tile(ui, entry, &response),
        // Loadable, so not dimmed; nothing to place, so no drag.
        EntryKind::InputMap => settings_tile(ui, rect, "INPUT"),
        EntryKind::SortingLayers => settings_tile(ui, rect, "LAYERS"),
        EntryKind::BusLayout => settings_tile(ui, rect, "BUSES"),
        EntryKind::RenderSettings => settings_tile(ui, rect, "RENDER"),
        // Dragged onto an animation player's clip field.
        EntryKind::Animation => badge_tile(
            ui,
            entry,
            &response,
            "ANIM",
            ui.visuals().widgets.active.bg_fill,
        ),
        // Dragged onto an animator's machine field.
        EntryKind::Animator => badge_tile(
            ui,
            entry,
            &response,
            "FSM",
            ui.visuals().widgets.active.bg_fill,
        ),
        EntryKind::Scene => scene::tile(editor, ui, frame, entry, &response),
        // Dragged onto a sprite's material field.
        EntryKind::Material => badge_tile(
            ui,
            entry,
            &response,
            "MAT",
            ui.visuals().widgets.hovered.bg_fill,
        ),
        // Listed, dimmed, and inert. Hiding it would read as a failed copy;
        // letting it be dragged would promise a loader that does not exist.
        EntryKind::Other => paint_block(ui, rect, ui.visuals().faint_bg_color),
    }

    paint_name(ui, rect, &entry.name);
}

/// Opens a sheet or an atlas in the Atlas Editor, saying why not if it
/// will not open.
fn open_in_atlas_editor(editor: &mut Editor, frame: &mut UiFrame<'_>, entry: &Entry) {
    if let Err(e) = editor.atlas_editor.open_path(frame, &entry.path) {
        log::error!("the Atlas Editor cannot open {}: {e}", entry.path.as_str());
    }
}

/// A texture tile: its own pixels, and the drag that carries its path.
///
/// The load is here rather than in the scan because it is the tile being drawn
/// that decides a texture is worth uploading: a directory of a thousand PNGs
/// costs the ones on screen, not all of them. `Textures` caches, so this is a
/// hash lookup on every frame after the first.
fn texture_tile(ui: &mut Ui, frame: &mut UiFrame<'_>, entry: &Entry, response: &egui::Response) {
    let handle = frame.textures.load(frame.device, frame.queue, &entry.path);

    let rect = picture(response.rect);
    match frame.thumbnail(handle) {
        Some(id) => {
            ui.painter().image(
                id,
                rect,
                // The whole texture, top-left to bottom-right.
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        // The frame a texture is first named: registration happens before the
        // layout, so the id exists next frame. A blank tile for one frame is
        // the whole cost.
        None => paint_block(ui, rect, ui.visuals().faint_bg_color),
    }

    drag_source(ui, entry, response);
}

/// Arms a tile as the source of a drag carrying its path.
///
/// The one gesture the inspector's path fields and the viewport's drop both
/// listen for, so every draggable tile has to arm it the same way.
fn drag_source(ui: &Ui, entry: &Entry, response: &egui::Response) {
    let response = response.interact(Sense::drag());
    response.dnd_set_drag_payload(entry.path.clone());
    if response.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        drag::ghost(ui, entry.path.as_str());
    }
}

/// An atlas tile: the first frame of the sheet it slices, and the same drag.
///
/// The sheet comes from the slicing's own texture hint, so a tile shows the
/// picture an author recognises rather than a generic file icon. A slicing
/// that names no sheet — or one that will not load — falls back to a block:
/// the file is there and the engine cannot picture it, which is what the
/// dimmed tile already says for everything else.
fn atlas_tile(ui: &mut Ui, frame: &mut UiFrame<'_>, entry: &Entry, response: &egui::Response) {
    let handle = frame.atlases.load(&entry.path);
    let atlas = frame.atlases.get(handle);
    let texture = atlas.texture().cloned();
    let uv = atlas.frame(0).and_then(|first| first.uv(atlas.sheet()));

    let rect = picture(response.rect);
    let thumbnail = texture
        .map(|path| frame.textures.load(frame.device, frame.queue, &path))
        .and_then(|handle| frame.thumbnail(handle));

    match (thumbnail, uv) {
        (Some(id), Some((min, max))) => {
            ui.painter().image(
                id,
                rect,
                Rect::from_min_max(egui::pos2(min[0], min[1]), egui::pos2(max[0], max[1])),
                Color32::WHITE,
            );
        }
        _ => paint_block(ui, rect, ui.visuals().faint_bg_color),
    }

    drag_source(ui, entry, response);
}

/// A sound tile: a block in the selection colour, since a waveform thumbnail
/// would mean decoding every file on screen, and the same drag as every other
/// loadable tile.
///
/// The colour rather than the dimmed fill is the point: dimmed already means
/// "this engine cannot load it", and a sound can be loaded.
fn clip_tile(ui: &mut Ui, entry: &Entry, response: &egui::Response) {
    // The format, which is what tells two takes of one sound apart, and which
    // every font egui ships can draw — a glyph like a note may not be.
    let format = entry
        .name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_uppercase())
        .unwrap_or_default();
    badge_tile(ui, entry, response, &format, ui.visuals().selection.bg_fill);
}

/// A font tile: a block saying "Aa", and the same drag. Dropped on the
/// viewport, a font becomes a label written in it.
///
/// Not the face itself: that would mean registering an egui font per file
/// on screen.
fn font_tile(ui: &mut Ui, entry: &Entry, response: &egui::Response) {
    badge_tile(
        ui,
        entry,
        response,
        "Aa",
        ui.visuals().widgets.hovered.bg_fill,
    );
}

/// A project file's tile — the input map, the sorting layers: a block with
/// its kind on it and no drag, since the game reads it rather than placing it.
fn settings_tile(ui: &mut Ui, rect: Rect, kind: &str) {
    paint_block(ui, rect, ui.visuals().widgets.noninteractive.bg_fill);
    ui.painter().text(
        picture(rect).center(),
        egui::Align2::CENTER_CENTER,
        kind,
        egui::TextStyle::Body.resolve(ui.style()),
        ui.visuals().text_color(),
    );
}

/// A tile that is a coloured block with `text` on it, armed as a drag.
pub(super) fn badge_tile(
    ui: &mut Ui,
    entry: &Entry,
    response: &egui::Response,
    text: &str,
    fill: Color32,
) {
    paint_block(ui, response.rect, fill);
    let rect = picture(response.rect);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::TextStyle::Heading.resolve(ui.style()),
        ui.visuals().strong_text_color(),
    );
    drag_source(ui, entry, response);
}

/// Takes a tile's worth of room and senses a click and a hover.
pub(super) fn allocate(ui: &mut Ui) -> (Rect, egui::Response) {
    ui.allocate_exact_size(Vec2::new(TILE, TILE + LABEL), Sense::click())
}

/// The part of a tile the picture goes in — everything above the name.
fn picture(rect: Rect) -> Rect {
    Rect::from_min_size(rect.min, Vec2::splat(TILE))
}

/// Fills a tile's picture area, for everything that has no picture of its own.
pub(super) fn paint_block(ui: &Ui, rect: Rect, color: Color32) {
    ui.painter()
        .rect_filled(picture(rect), egui::CornerRadius::same(3), color);
}

/// Writes the name under a tile, clipped to the tile's width.
///
/// Clipped rather than shortened with an ellipsis: the hover text already
/// carries the full path, and a run of names all ending in `…` says less than
/// their first ten characters do.
pub(super) fn paint_name(ui: &Ui, rect: Rect, name: &str) {
    let bottom = Rect::from_min_max(egui::pos2(rect.min.x, rect.max.y - LABEL), rect.max);
    ui.painter().with_clip_rect(bottom).text(
        bottom.center_top(),
        egui::Align2::CENTER_TOP,
        name,
        egui::TextStyle::Small.resolve(ui.style()),
        ui.visuals().text_color(),
    );
}
