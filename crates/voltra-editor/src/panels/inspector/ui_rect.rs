//! Where a screen-space element sits: anchors, offsets and the presets
//! that set both.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_render::glam::Vec2;
use voltra_scene::ui::Rect;
use voltra_scene::UiRect;

use super::active;

/// The anchor presets, as Godot's layout menu and Unity's anchor box offer
/// them. A preset keeps the element's current size and puts it 16 points in
/// from the anchored edges.
const PRESETS: [(&str, Vec2); 9] = [
    ("TL", Vec2::new(0.0, 0.0)),
    ("T", Vec2::new(0.5, 0.0)),
    ("TR", Vec2::new(1.0, 0.0)),
    ("L", Vec2::new(0.0, 0.5)),
    ("C", Vec2::new(0.5, 0.5)),
    ("R", Vec2::new(1.0, 0.5)),
    ("BL", Vec2::new(0.0, 1.0)),
    ("B", Vec2::new(0.5, 1.0)),
    ("BR", Vec2::new(1.0, 1.0)),
];

/// Margin a preset leaves between the element and the anchored edges.
const PRESET_MARGIN: f32 = 16.0;

/// `parent` is the box the element is anchored in, as the last frame laid
/// it out — what a preset needs to keep the element's size.
pub(super) fn show(ui: &mut Ui, rect: &mut UiRect, parent: Rect) -> Option<&'static str> {
    ui.label(RichText::new("UI Rect").strong())
        .on_hover_text("drawn on the screen, not in the world; points, y down");

    let mut claim = None;
    ui.horizontal_wrapped(|ui| {
        ui.label("anchor");
        for (index, (label, anchor)) in PRESETS.iter().enumerate() {
            if index % 3 == 0 && index > 0 {
                ui.add_space(4.0);
            }
            if ui.small_button(*label).clicked() {
                let size = rect.resolve(parent).size();
                let offset = Vec2::new(inward(anchor.x), inward(anchor.y)) * PRESET_MARGIN;
                *rect = UiRect::pinned(*anchor, offset, size);
                claim = Some("Anchor UI rect");
            }
        }
        if ui.small_button("fill").clicked() {
            *rect = UiRect::stretched(0.0);
            claim = Some("Anchor UI rect");
        }
    });

    egui::Grid::new("ui_rect").num_columns(3).show(ui, |ui| {
        claim = claim.or(pair(
            ui,
            "anchor min",
            &mut rect.anchor_min,
            0.01,
            Some(0.0..=1.0),
        ));
        claim = claim.or(pair(
            ui,
            "anchor max",
            &mut rect.anchor_max,
            0.01,
            Some(0.0..=1.0),
        ));
        claim = claim.or(pair(ui, "offset min", &mut rect.offset_min, 1.0, None));
        claim = claim.or(pair(ui, "offset max", &mut rect.offset_max, 1.0, None));
    });
    claim
}

/// Which way is away from the anchored edge on one axis: right or down from
/// the start, left or up from the end, nowhere from the middle.
fn inward(anchor: f32) -> f32 {
    if anchor < 0.5 {
        1.0
    } else if anchor > 0.5 {
        -1.0
    } else {
        0.0
    }
}

/// One labelled row of two drag values.
fn pair(
    ui: &mut Ui,
    label: &str,
    value: &mut Vec2,
    speed: f64,
    range: Option<std::ops::RangeInclusive<f32>>,
) -> Option<&'static str> {
    ui.label(label);
    let mut claim = None;
    for component in [&mut value.x, &mut value.y] {
        let mut drag = DragValue::new(component).speed(speed);
        if let Some(range) = range.clone() {
            drag = drag.range(range);
        }
        claim = claim.or(active(&ui.add(drag), "Edit UI rect"));
    }
    ui.end_row();
    claim
}
