//! A layer's animated frames: which frame cycles, through what, how fast.

use voltra_core::egui::{self, DragValue, RichText, TextEdit, Ui};
use voltra_scene::tilemap::TileAnimation;
use voltra_scene::Tilemap;

use super::super::active;

/// Every animation of `map`, editable, plus adding and removing one.
pub(super) fn show(ui: &mut Ui, map: &mut Tilemap) -> Option<&'static str> {
    ui.label(RichText::new("Animated tiles").strong())
        .on_hover_text("every cell showing the base frame cycles through the frames");
    let mut claim = None;
    let mut rekey: Option<(u32, u32)> = None;
    let mut remove: Option<u32> = None;
    for (&base, animation) in map.animations.iter_mut() {
        ui.push_id(base, |ui| {
            egui::Grid::new("tile_animation")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("base frame");
                    let mut key = base;
                    if active(&ui.add(DragValue::new(&mut key)), "Edit tile animation").is_some()
                        && key != base
                    {
                        rekey = Some((base, key));
                        claim = Some("Edit tile animation");
                    }
                    ui.end_row();

                    ui.label("frames");
                    claim = claim.or(frames_field(ui, animation));
                    ui.end_row();

                    ui.label("seconds each");
                    claim = claim.or(active(
                        &ui.add(
                            DragValue::new(&mut animation.frame_time)
                                .speed(0.01)
                                .range(0.01..=60.0),
                        ),
                        "Edit tile animation",
                    ));
                    ui.end_row();
                });
            if ui.small_button("Remove").clicked() {
                remove = Some(base);
            }
        });
    }
    if let Some((from, to)) = rekey {
        // A key already taken is left alone rather than overwritten.
        if !map.animations.contains_key(&to) {
            if let Some(animation) = map.animations.remove(&from) {
                map.animations.insert(to, animation);
            }
        }
    }
    if let Some(base) = remove {
        map.animations.remove(&base);
        claim = Some("Remove tile animation");
    }
    if ui.button("Add animation").clicked() {
        let base = (0..).find(|f| !map.animations.contains_key(f)).unwrap_or(0);
        map.animations.insert(
            base,
            TileAnimation {
                frames: vec![base],
                frame_time: 0.2,
            },
        );
        claim = Some("Add tile animation");
    }
    claim
}

/// The frames as `4, 5, 6`, committed when the field loses focus. Text that
/// is not a list of whole numbers is put back as it was.
fn frames_field(ui: &mut Ui, animation: &mut TileAnimation) -> Option<&'static str> {
    let committed = animation
        .frames
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let id = ui.id().with("frames");
    let mut text = ui.ctx().data_mut(|d| {
        d.get_temp_mut_or_insert_with::<String>(id, || committed.clone())
            .clone()
    });
    let response = ui.add(TextEdit::singleline(&mut text).hint_text("4, 5, 6"));
    let mut claim = None;
    if response.lost_focus() {
        match parse_frames(&text) {
            Some(frames) if !frames.is_empty() => {
                if frames != animation.frames {
                    animation.frames = frames;
                    claim = Some("Edit tile animation");
                }
            }
            _ => text = committed.clone(),
        }
    } else if !response.has_focus() {
        text = committed;
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, text));
    claim
}

fn parse_frames(text: &str) -> Option<Vec<u32>> {
    text.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| part.parse().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_frames;

    #[test]
    fn a_list_of_frames_parses_and_garbage_does_not() {
        assert_eq!(parse_frames("4, 5,6"), Some(vec![4, 5, 6]));
        assert_eq!(parse_frames("4, x"), None);
        assert_eq!(parse_frames(""), Some(vec![]));
    }
}
