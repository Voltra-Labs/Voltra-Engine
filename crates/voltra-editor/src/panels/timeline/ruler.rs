//! The time strip's scale, its ruler, and the playhead drawn across it.

use voltra_core::egui::{self, pos2, Color32, Rect, Stroke, Ui};

/// Room at either end of the strip, so a key at zero is not cut in half.
const PAD: f32 = 8.0;
/// Keys and the playhead land on hundredths of a second.
const SNAP: f32 = 100.0;

/// Seconds to x and back, over the strip.
#[derive(Clone, Copy)]
pub(super) struct Scale {
    pub(super) left: f32,
    pub(super) width: f32,
    pub(super) span: f32,
}

impl Scale {
    pub(super) fn new(strip: Rect, end: f32) -> Self {
        Self {
            left: strip.left() + PAD,
            width: (strip.width() - 2.0 * PAD).max(1.0),
            // A second at least, and a little past the end, so the last key
            // is not on the edge.
            span: (end * 1.1).max(1.0),
        }
    }

    pub(super) fn x(self, time: f32) -> f32 {
        self.left + time / self.span * self.width
    }

    pub(super) fn time(self, x: f32) -> f32 {
        let time = ((x - self.left) / self.width * self.span).max(0.0);
        (time * SNAP).round() / SNAP
    }
}

pub(super) fn playhead(ui: &Ui, strip: Rect, scale: Scale, time: f32) {
    let x = scale.x(time);
    if (strip.left()..=strip.right()).contains(&x) {
        ui.painter().line_segment(
            [pos2(x, strip.top()), pos2(x, strip.bottom())],
            Stroke::new(1.5, Color32::from_rgb(230, 90, 70)),
        );
    }
}

/// Ticks at a step that leaves room for their labels.
pub(super) fn draw_ruler(ui: &Ui, strip: Rect, scale: Scale) {
    let painter = ui.painter();
    painter.rect_filled(strip, 2.0, ui.visuals().faint_bg_color);
    let step = [0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]
        .into_iter()
        .find(|step| step / scale.span * scale.width >= 44.0)
        .unwrap_or(60.0);
    let font = egui::TextStyle::Small.resolve(ui.style());
    let color = ui.visuals().weak_text_color();
    let mut n = 0u32;
    loop {
        let time = n as f32 * step;
        let x = scale.x(time);
        if x > strip.right() || n > 1000 {
            break;
        }
        painter.line_segment(
            [pos2(x, strip.bottom() - 5.0), pos2(x, strip.bottom())],
            Stroke::new(1.0, color),
        );
        painter.text(
            pos2(x + 2.0, strip.top()),
            egui::Align2::LEFT_TOP,
            format!("{time}"),
            font.clone(),
            color,
        );
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_maps_to_x_and_back_on_hundredths() {
        let strip = Rect::from_min_max(pos2(100.0, 0.0), pos2(516.0, 20.0));
        let scale = Scale::new(strip, 2.0);
        for time in [0.0, 0.37, 1.5, 2.2] {
            assert!((scale.time(scale.x(time)) - time).abs() < 1e-4, "{time}");
        }
        assert_eq!(scale.time(0.0), 0.0, "never before the start");
        assert_eq!(Scale::new(strip, 0.0).span, 1.0, "a second at least");
    }
}
