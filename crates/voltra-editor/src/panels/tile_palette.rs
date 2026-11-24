//! The Tile Palette window: the selected layer's tileset laid out as its
//! sheet is; click a tile, or drag over a block of them, to paint with it.
//!
//! Unity's Tile Palette and Godot's TileMap panel, in a floating window so
//! the inspector stays the layer's settings. What it picks is the paint
//! stamp ([`crate::paint::stamp`]); the viewport paints it, and the
//! eyedropper (Alt-click in the viewport) picks one off the map instead.

use voltra_core::egui::{self, Color32, Rect, RichText, Sense, Stroke, StrokeKind, Ui};
use voltra_core::UiFrame;
use voltra_render::glam::UVec2;
use voltra_scene::Tilemap;

use crate::editor::Editor;
use crate::paint::palette::PaletteGrid;
use crate::paint::{Paint, PaintTool};

/// Side of one palette cell, in points.
const CELL: f32 = 32.0;

/// Whether the window is up, and the block being dragged out on it.
#[derive(Debug, Default)]
pub(crate) struct TilePaletteWindow {
    pub(crate) open: bool,
    /// The cell a drag started on.
    from: Option<UVec2>,
    /// The block last picked, outlined while the stamp is still it.
    picked: Option<(UVec2, UVec2)>,
}

impl TilePaletteWindow {
    /// Picks the palette block from `a` to `b` of `map`'s tileset as the
    /// stamp, and arms the brush if nothing is armed. What a drag here does;
    /// the bridge's `TilePick` too.
    pub(crate) fn pick(
        &mut self,
        paint: &mut Paint,
        grid: &PaletteGrid,
        a: UVec2,
        b: UVec2,
    ) -> Result<(), String> {
        let stamp = grid
            .stamp(a, b)
            .ok_or("no tile in that part of the palette")?;
        paint.stamp = stamp;
        self.picked = Some((a.min(b), a.max(b)));
        if paint.tool().is_none() {
            paint.set_tool(Some(PaintTool::Brush));
        }
        Ok(())
    }
}

pub(crate) fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    if !editor.tile_palette.open {
        return;
    }
    let mut open = true;
    egui::Window::new("Tile Palette")
        .open(&mut open)
        .default_size([360.0, 320.0])
        .show(ui.ctx(), |ui| {
            let layer = editor
                .selection
                .primary()
                .filter(|&e| frame.world.get::<Tilemap>(e).is_some());
            let Some(map) = layer.and_then(|layer| frame.world.get::<Tilemap>(layer)) else {
                ui.weak("Select a tilemap layer to paint.");
                return;
            };
            let Some(atlas) = map.atlas_handle.and_then(|h| frame.atlases.try_get(h)) else {
                ui.weak("The layer has no tileset: give it an atlas in the inspector.");
                return;
            };
            let grid = PaletteGrid::of(atlas);
            let image = map.texture_handle.and_then(|h| frame.thumbnail(h));
            let uvs: Vec<Option<Rect>> = atlas
                .frames()
                .iter()
                .map(|f| {
                    f.uv(atlas.sheet()).map(|(min, max)| {
                        Rect::from_min_max(egui::pos2(min[0], min[1]), egui::pos2(max[0], max[1]))
                    })
                })
                .collect();

            tools(ui, &mut editor.paint);
            turns(ui, &mut editor.paint);
            ui.separator();
            let window = &mut editor.tile_palette;
            let paint = &mut editor.paint;
            egui::ScrollArea::both().show(ui, |ui| {
                let size = grid.size().as_vec2() * CELL;
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(size.x, size.y), Sense::click_and_drag());
                let painter = ui.painter_at(rect);
                let cell_rect = |cell: UVec2| {
                    Rect::from_min_size(
                        rect.min + egui::vec2(cell.x as f32, cell.y as f32) * CELL,
                        egui::Vec2::splat(CELL),
                    )
                };
                for y in 0..grid.size().y {
                    for x in 0..grid.size().x {
                        let cell = UVec2::new(x, y);
                        let Some(index) = grid.frame_at(cell) else {
                            continue;
                        };
                        let at = cell_rect(cell).shrink(1.0);
                        match (image, uvs.get(index as usize).copied().flatten()) {
                            (Some(id), Some(uv)) => {
                                painter.image(id, at, uv, Color32::WHITE);
                            }
                            _ => {
                                painter.rect_filled(at, 2.0, ui.visuals().faint_bg_color);
                                painter.text(
                                    at.center(),
                                    egui::Align2::CENTER_CENTER,
                                    index.to_string(),
                                    egui::FontId::monospace(10.0),
                                    ui.visuals().text_color(),
                                );
                            }
                        }
                    }
                }

                let cell_at = |pos: egui::Pos2| {
                    let local = (pos - rect.min) / CELL;
                    let max = grid.size().saturating_sub(UVec2::ONE);
                    UVec2::new(local.x.max(0.0) as u32, local.y.max(0.0) as u32).min(max)
                };
                let pointer = response.interact_pointer_pos().map(cell_at);
                if response.drag_started() || (response.clicked() && window.from.is_none()) {
                    window.from = pointer;
                }
                let block = window.from.zip(pointer);
                if let Some((a, b)) = block {
                    let outline = cell_rect(a.min(b)).union(cell_rect(a.max(b)));
                    let stroke = Stroke::new(2.0, Color32::from_rgb(120, 230, 120));
                    painter.rect_stroke(outline, 0.0, stroke, StrokeKind::Inside);
                    if response.drag_stopped() || response.clicked() {
                        window.from = None;
                        if let Err(e) = window.pick(paint, &grid, a, b) {
                            log::info!("{e}");
                        }
                    }
                } else {
                    // The block the stamp still is, or the one tile it is —
                    // picked here, in the inspector or by the eyedropper.
                    let picked = window
                        .picked
                        .filter(|&(a, b)| grid.stamp(a, b).is_some_and(|s| s == paint.stamp))
                        .or_else(|| {
                            let tile = paint.stamp.single_tile()?;
                            let at = grid.position_of(tile.frame)?;
                            Some((at, at))
                        });
                    if let Some((a, b)) = picked {
                        let outline = cell_rect(a).union(cell_rect(b));
                        let stroke = Stroke::new(2.0, ui.visuals().selection.stroke.color);
                        painter.rect_stroke(outline, 0.0, stroke, StrokeKind::Inside);
                    }
                }
            });
        });
    editor.tile_palette.open = open;
}

/// The paint tools, as the toolbar has them, so the window is enough to
/// paint with.
fn tools(ui: &mut Ui, paint: &mut Paint) {
    ui.horizontal(|ui| {
        for tool in PaintTool::ALL {
            let armed = paint.tool() == Some(tool);
            if ui
                .selectable_label(armed, tool.glyph())
                .on_hover_text(tool.hint())
                .clicked()
            {
                paint.set_tool(if armed { None } else { Some(tool) });
            }
        }
        ui.label(RichText::new("Alt-click in the viewport picks").weak());
    });
}

/// The stamp's size, and the buttons that turn and mirror it — `Z`, `X`
/// and `Y` in the viewport do the same.
pub(crate) fn turns(ui: &mut Ui, paint: &mut Paint) {
    ui.horizontal(|ui| {
        let size = paint.stamp.size();
        ui.label(format!("stamp {}×{}", size.x, size.y));
        if ui
            .button("⟲")
            .on_hover_text("turn anticlockwise (Shift+Z)")
            .clicked()
        {
            paint.stamp = paint.stamp.rotated_ccw();
        }
        if ui.button("⟳").on_hover_text("turn clockwise (Z)").clicked() {
            paint.stamp = paint.stamp.rotated_cw();
        }
        if ui
            .button("⇔")
            .on_hover_text("mirror left to right (X)")
            .clicked()
        {
            paint.stamp = paint.stamp.mirrored_x();
        }
        if ui
            .button("⇕")
            .on_hover_text("mirror top to bottom (Y)")
            .clicked()
        {
            paint.stamp = paint.stamp.mirrored_y();
        }
    });
}
