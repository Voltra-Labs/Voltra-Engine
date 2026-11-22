//! Frame times as bars, newest on the right, against the 60 and 30 frames a
//! second lines.

use std::collections::VecDeque;

use voltra_core::egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use voltra_profile::Frame;

/// How many frames the graph shows: five seconds at 60 a second.
const SHOWN: usize = 300;
const HEIGHT: f32 = 80.0;
/// The budgets drawn across the graph, in milliseconds.
const BUDGETS: [f64; 2] = [1000.0 / 60.0, 1000.0 / 30.0];

pub(super) fn show(ui: &mut Ui, frames: &VecDeque<Frame>) {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, HEIGHT), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);

    let shown: Vec<&Frame> = frames
        .iter()
        .skip(frames.len().saturating_sub(SHOWN))
        .collect();
    // At least the 30 frames a second budget tall, so a calm graph does not
    // magnify noise into spikes.
    let top = shown
        .iter()
        .map(|frame| ms(frame))
        .fold(BUDGETS[1] * 1.2, f64::max);
    let y = |ms: f64| rect.bottom() - (ms / top) as f32 * rect.height();
    let bar = rect.width() / SHOWN as f32;
    let first = SHOWN - shown.len();

    for (i, frame) in shown.iter().enumerate() {
        let ms = ms(frame);
        let x = rect.left() + (first + i) as f32 * bar;
        let colour = if ms > BUDGETS[1] {
            Color32::from_rgb(220, 80, 70)
        } else if ms > BUDGETS[0] {
            Color32::from_rgb(230, 180, 60)
        } else {
            Color32::from_rgb(90, 170, 110)
        };
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(x, y(ms)),
                Pos2::new(x + bar.max(1.0), rect.bottom()),
            ),
            0.0,
            colour,
        );
    }
    for budget in BUDGETS {
        let line = y(budget);
        painter.line_segment(
            [Pos2::new(rect.left(), line), Pos2::new(rect.right(), line)],
            Stroke::new(1.0, ui.visuals().weak_text_color()),
        );
    }

    // The frame under the pointer, by its time.
    if let Some(pointer) = response.hover_pos() {
        let slot = ((pointer.x - rect.left()) / bar) as usize;
        if let Some(frame) = slot.checked_sub(first).and_then(|i| shown.get(i)) {
            response.on_hover_text(format!("frame {}: {:.2} ms", frame.index, ms(frame)));
        }
    }
}

fn ms(frame: &Frame) -> f64 {
    frame.duration.as_secs_f64() * 1000.0
}
