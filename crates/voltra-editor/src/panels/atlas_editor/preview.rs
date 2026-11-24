//! A run of regions played as an animation, to see a walk cycle walk
//! before a clip is made of it.

use voltra_core::egui::{self, Color32, DragValue, Rect, Sense, TextureId, Ui};

use crate::atlas_doc::AtlasDoc;

/// The largest a previewed frame is drawn, in points.
const SIZE: f32 = 96.0;

/// Which regions play, and how fast.
#[derive(Debug)]
pub(super) struct Preview {
    first: usize,
    last: usize,
    fps: f32,
    playing: bool,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            first: 0,
            last: 0,
            fps: 8.0,
            playing: true,
        }
    }
}

impl Preview {
    pub(super) fn show(&mut self, ui: &mut Ui, doc: &AtlasDoc, image: Option<TextureId>) {
        ui.label("Preview");
        let count = doc.regions().len();
        if count == 0 {
            ui.weak("No regions yet.");
            return;
        }
        ui.horizontal(|ui| {
            ui.label("frames");
            ui.add(DragValue::new(&mut self.first).range(0..=count - 1));
            ui.label("to");
            ui.add(DragValue::new(&mut self.last).range(0..=count - 1));
            ui.add(
                DragValue::new(&mut self.fps)
                    .range(0.5..=60.0)
                    .speed(0.2)
                    .suffix(" fps"),
            );
            ui.checkbox(&mut self.playing, "play");
        });
        let time = ui.input(|i| i.time);
        let index = frame_at(self.first, self.last, self.fps, self.playing, time).min(count - 1);
        if self.playing {
            ui.ctx().request_repaint_after_secs(1.0 / self.fps.max(0.5));
        }

        let region = &doc.regions()[index];
        let sheet = doc.sheet().as_vec2();
        let size = egui::vec2(region.frame.width as f32, region.frame.height as f32);
        let scale = (SIZE / size.x.max(size.y)).max(1.0 / 16.0);
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(SIZE), Sense::hover());
        ui.painter().rect_filled(rect, 0.0, Color32::from_gray(40));
        if let Some(id) = image {
            let uv = Rect::from_min_size(
                egui::pos2(
                    region.frame.x as f32 / sheet.x,
                    region.frame.y as f32 / sheet.y,
                ),
                egui::vec2(size.x / sheet.x, size.y / sheet.y),
            );
            let drawn = Rect::from_center_size(rect.center(), size * scale);
            ui.painter().image(id, drawn, uv, Color32::WHITE);
        }
        ui.weak(format!("frame {index} {}", region.name));
    }
}

/// The region shown `time` seconds in, cycling `first..=last` (either way
/// round) at `fps`; `first` when paused.
fn frame_at(first: usize, last: usize, fps: f32, playing: bool, time: f64) -> usize {
    let (low, high) = (first.min(last), first.max(last));
    if !playing || fps <= 0.0 || !fps.is_finite() {
        return low;
    }
    let len = high - low + 1;
    let step = (time * fps as f64).max(0.0) as usize;
    low + step % len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preview_cycles_its_frames_at_its_rate() {
        assert_eq!(frame_at(2, 4, 10.0, true, 0.0), 2);
        assert_eq!(frame_at(2, 4, 10.0, true, 0.25), 4);
        assert_eq!(frame_at(2, 4, 10.0, true, 0.31), 2, "and round again");
        assert_eq!(frame_at(4, 2, 10.0, true, 0.1), 3, "either way round");
        assert_eq!(frame_at(2, 4, 10.0, false, 9.0), 2, "paused on the first");
    }
}
