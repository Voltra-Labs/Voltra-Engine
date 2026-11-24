//! A curve over `0..=1` drawn as a graph its keys are dragged on.
//!
//! Unity's curve field and Godot's `Curve` editor, cut to keys with an
//! ease: drag a key to move it, double-click to add one, right-click to
//! remove one; the selected key's numbers and ease sit under the graph. The
//! value axis is `0` to `top` — the caller's, raised to the curve's highest
//! key, and held for the length of a drag so the graph does not rescale
//! under the pointer.

use voltra_core::egui::{self, Color32, DragValue, Pos2, Sense, Stroke, Ui, Vec2};
use voltra_scene::keyframes::Ease;
use voltra_scene::{Curve, CurveKey};

/// The label every change here goes into the history under.
pub(super) const EDIT: &str = "Edit curve";

/// How close, in points, a press must land to a key to take it.
const GRAB: f32 = 8.0;

/// Draws `curve` and its selected key's fields; `Some` while it is being
/// changed. `salt` separates two curves of one entity.
pub(crate) fn show(ui: &mut Ui, salt: &str, curve: &mut Curve, top: f32) -> Option<&'static str> {
    let id = ui.id().with(salt);
    let drag_id = id.with("drag");
    let selected_id = id.with("selected");
    let held: Option<(usize, f32)> = ui.ctx().data(|d| d.get_temp(drag_id));
    let top = held.map_or_else(|| axis_top(curve, top), |(_, top)| top);

    let width = ui.available_width().clamp(120.0, 240.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 64.0), Sense::click_and_drag());
    let to_screen = |t: f32, v: f32| {
        Pos2::new(
            rect.left() + t * rect.width(),
            rect.bottom() - (v / top).clamp(0.0, 1.0) * rect.height(),
        )
    };
    let from_screen = |p: Pos2| {
        (
            ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
            ((rect.bottom() - p.y) / rect.height() * top).clamp(0.0, top),
        )
    };
    let nearest = |p: Pos2, keys: &[CurveKey]| {
        keys.iter()
            .enumerate()
            .map(|(i, k)| (i, to_screen(k.t, k.value).distance(p)))
            .filter(|(_, d)| *d <= GRAB)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };

    let mut changed = false;
    let mut selected: Option<usize> = ui.ctx().data(|d| d.get_temp(selected_id));
    let pointer = response.interact_pointer_pos();
    if response.drag_started() {
        if let Some(i) = pointer.and_then(|p| nearest(p, &curve.keys)) {
            ui.ctx().data_mut(|d| {
                d.insert_temp(drag_id, (i, top));
            });
            selected = Some(i);
        }
    }
    if let (Some((i, _)), Some(p), true) = (held, pointer, response.dragged()) {
        if let Some(key) = curve.keys.get_mut(i) {
            let (t, value) = from_screen(p);
            (key.t, key.value) = (snap(t), snap(value));
            changed = true;
        }
    }
    if response.drag_stopped() {
        ui.ctx().data_mut(|d| d.remove::<(usize, f32)>(drag_id));
    }
    if response.clicked() {
        selected = pointer.and_then(|p| nearest(p, &curve.keys));
    }
    if response.double_clicked() {
        if let Some(p) = pointer.filter(|p| nearest(*p, &curve.keys).is_none()) {
            let (t, value) = from_screen(p);
            curve.keys.push(CurveKey::new(snap(t), snap(value)));
            selected = Some(curve.keys.len() - 1);
            changed = true;
        }
    }
    if response.secondary_clicked() {
        if let Some(i) = pointer.and_then(|p| nearest(p, &curve.keys)) {
            curve.keys.remove(i);
            selected = None;
            changed = true;
        }
    }
    let selected = selected.filter(|&i| i < curve.keys.len());

    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    painter.rect_filled(rect, 2.0, visuals.extreme_bg_color);
    if top > 1.0 {
        let one = to_screen(0.0, 1.0).y;
        painter.line_segment(
            [Pos2::new(rect.left(), one), Pos2::new(rect.right(), one)],
            Stroke::new(1.0, visuals.weak_text_color().gamma_multiply(0.4)),
        );
    }
    let points: Vec<Pos2> = (0..=64)
        .map(|i| {
            let t = i as f32 / 64.0;
            to_screen(t, curve.sample(t))
        })
        .collect();
    painter.line(points, Stroke::new(1.5, visuals.selection.stroke.color));
    for (i, key) in curve.keys.iter().enumerate() {
        let colour = if Some(i) == selected {
            Color32::from_rgb(255, 200, 60)
        } else {
            visuals.text_color()
        };
        painter.circle_filled(to_screen(key.t, key.value), 3.5, colour);
    }
    response
        .on_hover_text("drag a key to move it; double-click to add one; right-click to remove one");

    if let Some(i) = selected {
        let key = &mut curve.keys[i];
        ui.horizontal(|ui| {
            ui.label("t");
            changed |= ui
                .add(DragValue::new(&mut key.t).speed(0.01).range(0.0..=1.0))
                .changed();
            ui.label("value");
            changed |= ui.add(DragValue::new(&mut key.value).speed(0.01)).changed();
            egui::ComboBox::from_id_salt(id.with("ease"))
                .selected_text(key.ease.label())
                .show_ui(ui, |ui| {
                    for ease in Ease::ALL {
                        changed |= ui
                            .selectable_value(&mut key.ease, ease, ease.label())
                            .changed();
                    }
                });
        });
    }
    ui.ctx().data_mut(|d| match selected {
        Some(i) => {
            d.insert_temp(selected_id, i);
        }
        None => d.remove::<usize>(selected_id),
    });
    (changed || held.is_some()).then_some(EDIT)
}

/// The top of the value axis: `top`, or the curve's highest key rounded up
/// to a whole number when that is higher.
pub(super) fn axis_top(curve: &Curve, top: f32) -> f32 {
    let (_, hi) = curve.range();
    let top = if hi.is_finite() {
        top.max(hi.ceil())
    } else {
        top
    };
    top.max(f32::EPSILON)
}

/// A dragged number, to hundredths: what a person can read back.
fn snap(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_axis_grows_to_the_highest_key() {
        assert_eq!(axis_top(&Curve::linear(0.0, 1.0), 1.0), 1.0);
        assert_eq!(axis_top(&Curve::linear(0.0, 2.3), 1.0), 3.0);
        assert_eq!(axis_top(&Curve::default(), 2.0), 2.0);
    }

    #[test]
    fn a_dragged_number_is_kept_to_hundredths() {
        assert_eq!(snap(0.12345), 0.12);
    }
}
