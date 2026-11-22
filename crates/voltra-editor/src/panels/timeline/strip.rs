//! The ruler and one row per track: its name, its two buttons, and its keys
//! as diamonds on a strip of time under one playhead.

use voltra_core::egui::{self, pos2, vec2, Pos2, Rect, Sense, Stroke, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::keyframes::target;
use voltra_scene::AnimationClip;

use super::key::resort;
use super::ruler::{draw_ruler, playhead, Scale};
use crate::timeline::Timeline;

/// Width of the names and buttons left of the strip.
const LABELS: f32 = 210.0;
/// Height of a row.
const ROW: f32 = 20.0;
/// Answers whether the clip changed.
pub(super) fn show(
    ui: &mut Ui,
    timeline: &mut Timeline,
    world: &World,
    root: Entity,
    clip: &mut AnimationClip,
) -> bool {
    let end = clip.duration();
    let (row, ruler) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click_and_drag());
    let strip = Rect::from_min_max(pos2(row.left() + LABELS, row.top()), row.max);
    let scale = Scale::new(strip, end);
    draw_ruler(ui, strip, scale);
    if let Some(at) = ruler
        .interact_pointer_pos()
        .filter(|_| ruler.is_pointer_button_down_on() || ruler.clicked())
    {
        timeline.playhead = scale.time(at.x);
    }
    playhead(ui, strip, scale, timeline.playhead);

    if clip.tracks.is_empty() {
        ui.label(
            egui::RichText::new("no tracks yet: add one below")
                .weak()
                .small(),
        );
    }
    let mut changed = false;
    let mut remove = None;
    for t in 0..clip.tracks.len() {
        let (row, background) =
            ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
        let strip = Rect::from_min_max(pos2(row.left() + LABELS, row.top()), row.max);
        let track = &mut clip.tracks[t];
        let found = target::resolve(world, root, &track.target);
        let name = if track.target.is_empty() {
            "self"
        } else {
            &track.target
        };
        let missing = found.is_none_or(|e| track.property.read(world, e).is_none());
        let text = if missing {
            ui.visuals().weak_text_color()
        } else {
            ui.visuals().text_color()
        };
        ui.painter().text(
            pos2(row.left() + 4.0, row.center().y),
            egui::Align2::LEFT_CENTER,
            format!("{name} · {}", track.property.label()),
            egui::TextStyle::Small.resolve(ui.style()),
            text,
        );
        let button = |offset: f32| {
            Rect::from_min_size(
                pos2(row.left() + LABELS - offset, row.top() + 1.0),
                vec2(30.0, ROW - 2.0),
            )
        };
        let key_here = ui
            .put(button(68.0), egui::Button::new("key").small())
            .on_hover_text("key the value it has now, at the playhead");
        if key_here.clicked() {
            match found.and_then(|e| track.property.read(world, e)) {
                Some(value) => {
                    let k = track.set_key(timeline.playhead, value);
                    timeline.selected = Some((t, k));
                    changed = true;
                }
                None => log::warn!("{name} has no {} to key", track.property.label()),
            }
        }
        if ui
            .put(button(34.0), egui::Button::new("del").small())
            .on_hover_text("remove the track")
            .clicked()
        {
            remove = Some(t);
        }

        ui.painter()
            .rect_filled(strip, 2.0, ui.visuals().extreme_bg_color);
        if background.clicked() {
            if let Some(at) = background
                .interact_pointer_pos()
                .filter(|p| strip.contains(*p))
            {
                timeline.playhead = scale.time(at.x);
                timeline.selected = None;
            }
        }
        changed |= keys(ui, timeline, t, track, strip, scale);
        playhead(ui, strip, scale, timeline.playhead);
    }
    if let Some(t) = remove {
        clip.tracks.remove(t);
        timeline.selected = None;
        changed = true;
    }
    changed
}

/// One track's diamonds: a click selects one and moves the playhead to it,
/// a drag moves it in time, a right click deletes it.
fn keys(
    ui: &mut Ui,
    timeline: &mut Timeline,
    t: usize,
    track: &mut voltra_scene::keyframes::Track,
    strip: Rect,
    scale: Scale,
) -> bool {
    let mut changed = false;
    let mut dropped = None;
    let mut deleted = None;
    let y = strip.center().y;
    for (k, key) in track.keys.iter_mut().enumerate() {
        let at = pos2(scale.x(key.time), y);
        let id = ui.id().with(("timeline_key", t, k));
        let response = ui.interact(
            Rect::from_center_size(at, vec2(12.0, 12.0)),
            id,
            Sense::click_and_drag(),
        );
        if response.clicked() || response.drag_started() {
            timeline.selected = Some((t, k));
            timeline.playhead = key.time;
        }
        if response.dragged() {
            if let Some(pointer) = response.interact_pointer_pos() {
                let time = scale.time(pointer.x);
                if time != key.time {
                    key.time = time;
                    timeline.playhead = time;
                    changed = true;
                }
            }
        }
        // Put in order once let go: sorting under the pointer would change
        // the key's index, and with it the id egui is tracking the drag by.
        if response.drag_stopped() {
            dropped = Some((k, key.time));
        }
        if response.secondary_clicked() {
            deleted = Some(k);
        }
        let selected = timeline.selected == Some((t, k));
        diamond(ui, at, selected, response.hovered());
    }
    if let Some((k, time)) = dropped {
        timeline.selected = Some((t, resort(track, k, time)));
    }
    if let Some(k) = deleted {
        track.keys.remove(k);
        timeline.selected = None;
        changed = true;
    }
    changed
}

fn diamond(ui: &Ui, at: Pos2, selected: bool, hovered: bool) {
    let r = if hovered { 6.0 } else { 5.0 };
    let fill = if selected {
        ui.visuals().selection.bg_fill
    } else {
        ui.visuals().strong_text_color()
    };
    let points = vec![
        at + vec2(0.0, -r),
        at + vec2(r, 0.0),
        at + vec2(0.0, r),
        at + vec2(-r, 0.0),
    ];
    ui.painter()
        .add(egui::Shape::convex_polygon(points, fill, Stroke::NONE));
}
