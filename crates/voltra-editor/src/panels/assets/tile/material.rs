//! A material's tile: a surface material drawn on a lit sphere, a sprite
//! material as a badge.

use voltra_assets::browse::Entry;
use voltra_core::egui::{self, Color32, Rect, Ui};
use voltra_core::UiFrame;

use super::{badge_tile, drag_source, paint_block, picture};

/// A surface material's tile is the material on a sphere — the
/// inspector's preview, small, drawn offscreen as a texture's thumbnail is
/// uploaded: once, and again only when the file or an image it names
/// changes. A sprite material, or one that cannot be read, is a badge.
/// Either drags its path onto a mesh's or a sprite's material field.
pub(super) fn tile(ui: &mut Ui, frame: &mut UiFrame<'_>, entry: &Entry, response: &egui::Response) {
    let source = frame.materials.as_deref_mut().and_then(|materials| {
        let handle = materials.load(&entry.path);
        materials.get(handle).and_then(|m| m.surface()).cloned()
    });
    let Some(source) = source else {
        badge_tile(
            ui,
            entry,
            response,
            "MAT",
            ui.visuals().widgets.hovered.bg_fill,
        );
        return;
    };
    let rect = picture(response.rect);
    let pixels = (rect.width() * ui.ctx().pixels_per_point()).round() as u32;
    let key = format!("browser/{}", entry.path.as_str());
    match frame.surface_preview(&key, pixels, &source) {
        Some(id) => {
            paint_block(ui, rect, ui.visuals().extreme_bg_color);
            ui.painter().image(
                id,
                rect,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        None => paint_block(ui, rect, ui.visuals().faint_bg_color),
    }
    drag_source(ui, entry, response);
}
