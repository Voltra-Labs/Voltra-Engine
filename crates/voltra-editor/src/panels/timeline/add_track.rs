//! The row that adds a track: which entity, which of its properties, keyed
//! with the value it has now at the playhead.

use voltra_core::egui::{self, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::keyframes::{target, Property, Track};
use voltra_scene::AnimationClip;

use crate::timeline::Timeline;

/// Answers whether the clip changed.
pub(super) fn show(
    ui: &mut Ui,
    timeline: &mut Timeline,
    world: &World,
    root: Entity,
    clip: &mut AnimationClip,
) -> bool {
    let entities = target::paths(world, root);
    // A target that is no longer there — renamed, or another clip's — falls
    // back to the player itself.
    let Some(&(_, entity)) = entities
        .iter()
        .find(|(path, _)| *path == timeline.new_target)
        .or(entities.first())
    else {
        return false;
    };
    if !entities
        .iter()
        .any(|(path, _)| *path == timeline.new_target)
    {
        timeline.new_target.clear();
    }
    let free = free_properties(world, entity, &timeline.new_target, clip);
    if timeline.new_property.is_none_or(|p| !free.contains(&p)) {
        timeline.new_property = free.first().copied();
    }

    let mut added = false;
    ui.horizontal(|ui| {
        ui.label("add track");
        egui::ComboBox::from_id_salt("track_target")
            .selected_text(shown(&timeline.new_target))
            .show_ui(ui, |ui| {
                for (path, _) in &entities {
                    ui.selectable_value(&mut timeline.new_target, path.clone(), shown(path));
                }
            })
            .response
            .on_hover_text("the player's entity, or a named child under it");
        egui::ComboBox::from_id_salt("track_property")
            .selected_text(
                timeline
                    .new_property
                    .map_or("nothing to animate", Property::label),
            )
            .show_ui(ui, |ui| {
                for &property in &free {
                    ui.selectable_value(
                        &mut timeline.new_property,
                        Some(property),
                        property.label(),
                    );
                }
            });
        let Some(property) = timeline.new_property else {
            return;
        };
        if ui.button("Add").clicked() {
            let mut track = Track::new(timeline.new_target.clone(), property);
            if let Some(value) = property.read(world, entity) {
                track.set_key(timeline.playhead, value);
            }
            clip.tracks.push(track);
            timeline.selected = Some((clip.tracks.len() - 1, 0));
            added = true;
        }
    });
    added
}

/// What a target path is called in a list.
fn shown(path: &str) -> &str {
    if path.is_empty() {
        "self"
    } else {
        path
    }
}

/// The properties `entity` has that the clip does not yet animate on it.
fn free_properties(
    world: &World,
    entity: Entity,
    path: &str,
    clip: &AnimationClip,
) -> Vec<Property> {
    Property::ALL
        .into_iter()
        .filter(|p| p.read(world, entity).is_some())
        .filter(|p| {
            !clip
                .tracks
                .iter()
                .any(|t| t.target == path && t.property == *p)
        })
        .collect()
}
