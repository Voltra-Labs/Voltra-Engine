//! The selected key: its time, its value and its ease, and its removal.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::keyframes::{Ease, Property, Track, Value};
use voltra_scene::AnimationClip;

use crate::timeline::Timeline;

/// The row for the selected key, or a hint. Answers whether the clip
/// changed.
pub(super) fn show(ui: &mut Ui, timeline: &mut Timeline, clip: &mut AnimationClip) -> bool {
    let Some((t, k)) = timeline.selected else {
        ui.label(
            RichText::new("click a key to edit it, right-click to delete it; key keys the current value at the playhead")
                .weak()
                .small(),
        );
        return false;
    };
    let Some(track) = clip.tracks.get_mut(t) else {
        timeline.selected = None;
        return false;
    };
    let property = track.property;
    let Some(key) = track.keys.get_mut(k) else {
        timeline.selected = None;
        return false;
    };

    let mut changed = false;
    let mut retimed = false;
    let mut delete = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} key", property.label())).strong());
        ui.label("at");
        if ui
            .add(
                DragValue::new(&mut key.time)
                    .speed(0.01)
                    .range(0.0..=3600.0)
                    .suffix("s"),
            )
            .changed()
        {
            changed = true;
            retimed = true;
        }
        changed |= value_ui(ui, property, &mut key.value);
        egui::ComboBox::from_id_salt("key_ease")
            .selected_text(key.ease.label())
            .show_ui(ui, |ui| {
                for ease in Ease::ALL {
                    changed |= ui
                        .selectable_value(&mut key.ease, ease, ease.label())
                        .changed();
                }
            })
            .response
            .on_hover_text("how the value moves from this key to the next");
        delete = ui.button("Delete key").clicked();
    });

    if delete {
        track.keys.remove(k);
        timeline.selected = None;
        return true;
    }
    if retimed {
        let time = track.keys[k].time;
        timeline.selected = Some((t, resort(track, k, time)));
    }
    changed
}

/// Puts the key at `index`, now at `time`, back in order. Answers where it
/// went.
pub(super) fn resort(track: &mut Track, index: usize, time: f32) -> usize {
    let key = track.keys.remove(index);
    let at = track.keys.partition_point(|k| k.time <= time);
    track.keys.insert(at, key);
    at
}

/// A widget for one value, of its kind. Rotation is shown in degrees, as
/// the transform's own row shows it.
pub(super) fn value_ui(ui: &mut Ui, property: Property, value: &mut Value) -> bool {
    match value {
        Value::Float(v) if property == Property::Rotation2D => {
            let mut degrees = v.to_degrees();
            let changed = ui
                .add(DragValue::new(&mut degrees).speed(0.5).suffix("°"))
                .changed();
            if changed {
                *v = degrees.to_radians();
            }
            changed
        }
        Value::Float(v) => ui.add(DragValue::new(v).speed(0.01)).changed(),
        Value::Vec2(v) => {
            let x = ui
                .add(DragValue::new(&mut v.x).speed(0.01).prefix("x "))
                .changed();
            let y = ui
                .add(DragValue::new(&mut v.y).speed(0.01).prefix("y "))
                .changed();
            x || y
        }
        Value::Color(c) => ui.color_edit_button_rgba_unmultiplied(c).changed(),
        Value::Int(i) => ui
            .add(DragValue::new(i).range(0..=i64::from(u32::MAX)))
            .changed(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_retimed_key_goes_back_in_order() {
        let mut track = Track::new("", Property::Rotation2D);
        for t in [0.0, 1.0, 2.0] {
            track.set_key(t, Value::Float(t));
        }
        track.keys[0].time = 1.5;
        assert_eq!(resort(&mut track, 0, 1.5), 1);
        let times: Vec<f32> = track.keys.iter().map(|k| k.time).collect();
        assert_eq!(times, [1.0, 1.5, 2.0]);
        assert_eq!(
            track.keys[1].value,
            Value::Float(0.0),
            "the key moved, not its value"
        );
    }
}
