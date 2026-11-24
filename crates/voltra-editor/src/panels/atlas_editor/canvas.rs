//! The sheet, with its regions outlined: click one to select it, or drag
//! out a new one while drawing.

use voltra_assets::Frame;
use voltra_core::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, TextureId, Ui};
use voltra_render::glam::UVec2;

use crate::atlas_doc::AtlasDoc;

/// The largest the sheet is drawn, in points, before it scrolls.
const VIEW: f32 = 480.0;

const REGION: Color32 = Color32::from_rgb(90, 170, 255);
const SELECTED: Color32 = Color32::from_rgb(255, 200, 60);
const DRAWING: Color32 = Color32::from_rgb(120, 230, 120);

/// How the sheet is shown and what is being dragged on it.
#[derive(Debug)]
pub(super) struct Canvas {
    /// Whether a drag on the sheet adds a region.
    pub(super) draw: bool,
    /// Points per texel.
    zoom: f32,
    /// The texel a region drag started on.
    from: Option<UVec2>,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            draw: false,
            zoom: 0.0,
            from: None,
        }
    }
}

impl Canvas {
    pub(super) fn show(&mut self, ui: &mut Ui, doc: &mut AtlasDoc, image: Option<TextureId>) {
        let sheet = doc.sheet().as_vec2();
        if self.zoom <= 0.0 {
            // Fit the sheet on first sight, in whole steps so texels stay
            // square: a 16×16 sheet is drawn big, a 2048² one small.
            let fit = (VIEW / sheet.x.max(sheet.y)).max(1.0 / 16.0);
            self.zoom = if fit >= 1.0 { fit.floor() } else { fit };
        }
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Zoom");
                ui.add(
                    egui::DragValue::new(&mut self.zoom)
                        .range(1.0 / 16.0..=32.0)
                        .speed(0.05)
                        .suffix("×"),
                );
            });
            egui::ScrollArea::both()
                .id_salt("atlas_canvas")
                .max_width(VIEW)
                .max_height(VIEW)
                .show(ui, |ui| self.sheet(ui, doc, image));
        });
    }

    fn sheet(&mut self, ui: &mut Ui, doc: &mut AtlasDoc, image: Option<TextureId>) {
        let size = doc.sheet().as_vec2() * self.zoom;
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(size.x, size.y), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_gray(40));
        match image {
            Some(id) => {
                let uv = Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0));
                painter.image(id, rect, uv, Color32::WHITE);
            }
            None => {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "loading…",
                    egui::FontId::default(),
                    Color32::GRAY,
                );
            }
        }

        let to_screen = |frame: Frame| {
            Rect::from_min_size(
                rect.min + egui::vec2(frame.x as f32, frame.y as f32) * self.zoom,
                egui::vec2(frame.width as f32, frame.height as f32) * self.zoom,
            )
        };
        for (index, region) in doc.regions().iter().enumerate() {
            let selected = doc.selected == Some(index);
            let color = if selected { SELECTED } else { REGION };
            let on_screen = to_screen(region.frame);
            painter.rect_stroke(on_screen, 0.0, Stroke::new(1.0, color), StrokeKind::Inside);
            if selected {
                let pivot = region
                    .pivot
                    .unwrap_or(voltra_render::glam::Vec2::splat(0.5));
                let at = on_screen.min + on_screen.size() * egui::vec2(pivot.x, pivot.y);
                painter.circle_stroke(at, 3.0, Stroke::new(1.5, SELECTED));
            }
        }

        let texel = |pos: Pos2| {
            let local = (pos - rect.min) / self.zoom;
            let max = doc.sheet();
            UVec2::new(
                (local.x.max(0.0) as u32).min(max.x),
                (local.y.max(0.0) as u32).min(max.y),
            )
        };
        let pointer = response.interact_pointer_pos();
        if self.draw {
            if response.drag_started() {
                self.from = pointer.map(texel);
            }
            if let (Some(from), Some(pointer)) = (self.from, pointer) {
                let drawn = span(from, texel(pointer));
                painter.rect_stroke(
                    to_screen(drawn),
                    0.0,
                    Stroke::new(1.5, DRAWING),
                    StrokeKind::Inside,
                );
                if response.drag_stopped() {
                    self.from = None;
                    if !drawn.is_empty() {
                        if let Err(e) = doc.add(drawn, None) {
                            log::warn!("{e}");
                        }
                    }
                }
            }
        } else if response.clicked() {
            if let Some(at) = pointer.map(texel) {
                // The last drawn, smallest-first would hide a region inside
                // another; the top of the list is what is outlined last.
                doc.selected = doc
                    .regions()
                    .iter()
                    .rposition(|region| contains(region.frame, at));
            }
        }
    }
}

/// The rectangle of texels from `a` to `b`, either way round, `b` included.
fn span(a: UVec2, b: UVec2) -> Frame {
    let min = a.min(b);
    let max = a.max(b);
    Frame::new(min.x, min.y, max.x - min.x + 1, max.y - min.y + 1)
}

fn contains(frame: Frame, at: UVec2) -> bool {
    at.x >= frame.x
        && at.y >= frame.y
        && at.x < frame.x + frame.width
        && at.y < frame.y + frame.height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_spans_both_ends_whichever_way_it_went() {
        assert_eq!(
            span(UVec2::new(5, 2), UVec2::new(1, 4)),
            Frame::new(1, 2, 5, 3)
        );
        assert_eq!(
            span(UVec2::splat(3), UVec2::splat(3)),
            Frame::new(3, 3, 1, 1)
        );
    }

    #[test]
    fn a_frame_holds_its_texels_and_not_the_next() {
        let frame = Frame::new(16, 0, 16, 16);
        assert!(contains(frame, UVec2::new(16, 15)));
        assert!(!contains(frame, UVec2::new(32, 0)));
    }
}
