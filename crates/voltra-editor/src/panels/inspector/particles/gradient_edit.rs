//! A gradient drawn as a bar whose stops are dragged along it.
//!
//! The curve editor's gestures on one axis: drag a stop to move it,
//! double-click the bar to add one in the colour it already has there,
//! right-click a stop to remove it; the selected stop's place and colour sit
//! under the bar.

use voltra_core::egui::{
    self, epaint::Mesh, Color32, DragValue, Pos2, Rect, Rgba, Sense, Stroke, Ui, Vec2,
};
use voltra_scene::{Gradient, GradientKey};

/// The label every change here goes into the history under.
const EDIT: &str = "Edit gradient";

/// How close, in points, a press must land to a stop to take it.
const GRAB: f32 = 6.0;

/// Draws `gradient` and its selected stop's fields; `Some` while it is being
/// changed.
pub(super) fn show(ui: &mut Ui, salt: &str, gradient: &mut Gradient) -> Option<&'static str> {
    let id = ui.id().with(salt);
    let drag_id = id.with("drag");
    let selected_id = id.with("selected");
    let held: Option<usize> = ui.ctx().data(|d| d.get_temp(drag_id));

    let width = ui.available_width().clamp(120.0, 240.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 28.0), Sense::click_and_drag());
    let bar = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.min.y + 18.0));
    let x_of = |t: f32| bar.left() + t.clamp(0.0, 1.0) * bar.width();
    let t_of = |x: f32| ((x - bar.left()) / bar.width()).clamp(0.0, 1.0);
    let nearest = |p: Pos2, keys: &[GradientKey]| {
        keys.iter()
            .enumerate()
            .map(|(i, k)| (i, (x_of(k.t) - p.x).abs()))
            .filter(|(_, d)| *d <= GRAB)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };

    let mut changed = false;
    let mut selected: Option<usize> = ui.ctx().data(|d| d.get_temp(selected_id));
    let pointer = response.interact_pointer_pos();
    if response.drag_started() {
        if let Some(i) = pointer.and_then(|p| nearest(p, &gradient.keys)) {
            ui.ctx().data_mut(|d| {
                d.insert_temp(drag_id, i);
            });
            selected = Some(i);
        }
    }
    if let (Some(i), Some(p), true) = (held, pointer, response.dragged()) {
        if let Some(key) = gradient.keys.get_mut(i) {
            key.t = (t_of(p.x) * 100.0).round() / 100.0;
            changed = true;
        }
    }
    if response.drag_stopped() {
        ui.ctx().data_mut(|d| d.remove::<usize>(drag_id));
    }
    if response.clicked() {
        selected = pointer.and_then(|p| nearest(p, &gradient.keys));
    }
    if response.double_clicked() {
        if let Some(p) = pointer.filter(|p| nearest(*p, &gradient.keys).is_none()) {
            let t = (t_of(p.x) * 100.0).round() / 100.0;
            let color = gradient.sample(t).unwrap_or([1.0; 4]);
            gradient.keys.push(GradientKey::new(t, color));
            selected = Some(gradient.keys.len() - 1);
            changed = true;
        }
    }
    if response.secondary_clicked() {
        if let Some(i) = pointer.and_then(|p| nearest(p, &gradient.keys)) {
            gradient.keys.remove(i);
            selected = None;
            changed = true;
        }
    }
    let selected = selected.filter(|&i| i < gradient.keys.len());

    let painter = ui.painter_at(rect);
    painter.rect_filled(bar, 0.0, ui.visuals().extreme_bg_color);
    let mut mesh = Mesh::default();
    const STRIPS: usize = 48;
    for i in 0..=STRIPS {
        let t = i as f32 / STRIPS as f32;
        let [r, g, b, a] = gradient.sample(t).unwrap_or([0.0; 4]);
        let colour: Color32 = Rgba::from_rgba_unmultiplied(r, g, b, a.clamp(0.0, 1.0)).into();
        mesh.colored_vertex(Pos2::new(x_of(t), bar.top()), colour);
        mesh.colored_vertex(Pos2::new(x_of(t), bar.bottom()), colour);
        if i > 0 {
            let base = (i as u32 - 1) * 2;
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base + 1, base + 3, base + 2);
        }
    }
    painter.add(mesh);
    for (i, key) in gradient.keys.iter().enumerate() {
        let x = x_of(key.t);
        let stroke = if Some(i) == selected {
            Stroke::new(2.0, Color32::from_rgb(255, 200, 60))
        } else {
            Stroke::new(1.0, ui.visuals().text_color())
        };
        painter.line_segment(
            [Pos2::new(x, bar.top()), Pos2::new(x, rect.bottom())],
            stroke,
        );
    }
    response.on_hover_text(
        "drag a stop to move it; double-click to add one; right-click to remove one",
    );

    if let Some(i) = selected {
        let key = &mut gradient.keys[i];
        ui.horizontal(|ui| {
            ui.label("t");
            changed |= ui
                .add(DragValue::new(&mut key.t).speed(0.01).range(0.0..=1.0))
                .changed();
            let popup = ui.auto_id_with("popup");
            let response = crate::panels::colour_edit::rgba(ui, &mut key.color);
            // Open is an edit in progress, as a held drag is: one entry
            // however long the picker stays up.
            changed |= egui::Popup::is_id_open(ui.ctx(), popup) || response.changed();
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
