//! The tileset as a strip of its frames: click one to paint with it. The
//! Tile Palette window shows it laid out as the sheet is, for blocks.
//!
//! Unity's Tile Palette and Godot's TileSet panel both show the sheet cut
//! into its tiles rather than a frame number, because "which one is the
//! corner piece" is answered by looking, not by counting.

use voltra_assets::Atlas;
use voltra_core::egui::{self, Color32, Rect, RichText, Sense, Stroke, Ui};
use voltra_core::ui::TextureId;
use voltra_scene::Tile;

use crate::paint::{Paint, PaintTool, Stamp};

/// Side of one tile button, in points.
const TILE: f32 = 32.0;
/// How tall the palette may grow before it scrolls, in points. A sheet of
/// hundreds of tiles should not push the rest of the inspector off screen.
const MAX_HEIGHT: f32 = 240.0;

/// What the palette draws with and writes to.
pub(in crate::panels::inspector) struct Palette<'a> {
    /// The sheet's thumbnail, if it is loaded.
    pub sheet: Option<TextureId>,
    pub paint: &'a mut Paint,
    /// Whether the Tile Palette window is up.
    pub window: &'a mut bool,
}

/// Draws the palette and the stamp's turns; a click picks a one-tile stamp
/// and arms the brush if no paint tool is armed yet.
///
/// Not a scene edit: the brush is editor state, so nothing here claims the
/// history.
pub(super) fn show(ui: &mut Ui, atlas: &Atlas, palette: Palette<'_>) {
    let Palette {
        sheet,
        paint,
        window,
    } = palette;
    ui.label(RichText::new("Palette").strong());
    if atlas.is_empty() {
        ui.label(RichText::new("the tileset has no tiles").weak());
        return;
    }

    let selected_stroke = Stroke::new(2.0, ui.visuals().selection.stroke.color);
    egui::ScrollArea::vertical()
        .id_salt("palette")
        .max_height(MAX_HEIGHT)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (index, frame) in atlas.frames().iter().enumerate() {
                    let index = index as u32;
                    let (rect, response) =
                        ui.allocate_exact_size(egui::Vec2::splat(TILE), Sense::click());
                    match (sheet, frame.uv(atlas.sheet())) {
                        (Some(id), Some((min, max))) => {
                            let uv = Rect::from_min_max(
                                egui::pos2(min[0], min[1]),
                                egui::pos2(max[0], max[1]),
                            );
                            ui.painter().image(id, rect, uv, Color32::WHITE);
                        }
                        // The sheet is not loaded, or the frame is off it: the
                        // number is still something to click.
                        _ => {
                            ui.painter()
                                .rect_filled(rect, 2.0, ui.visuals().faint_bg_color);
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                index.to_string(),
                                egui::FontId::monospace(10.0),
                                ui.visuals().text_color(),
                            );
                        }
                    }
                    if paint.stamp.single_tile().is_some_and(|t| t.frame == index) {
                        ui.painter().rect_stroke(
                            rect,
                            0.0,
                            selected_stroke,
                            egui::StrokeKind::Inside,
                        );
                    }
                    if response.on_hover_text(format!("tile {index}")).clicked() {
                        paint.stamp = Stamp::single(Tile::new(index));
                        if paint.tool().is_none() {
                            paint.set_tool(Some(PaintTool::Brush));
                        }
                    }
                }
            });
        });

    crate::panels::tile_palette::turns(ui, paint);
    if ui
        .button("Tile Palette…")
        .on_hover_text("the sheet laid out as it is: drag over it to pick a block")
        .clicked()
    {
        *window = true;
    }
    ui.label(RichText::new("B brush · U rectangle · D eraser · G fill · Alt-click picks").weak());
}
