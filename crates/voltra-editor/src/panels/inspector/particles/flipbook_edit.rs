//! A particle's flipbook: the atlas, which of its frames, and how fast.

use voltra_assets::Atlases;
use voltra_core::egui::{self, DragValue, RichText, TextEdit, Ui};
use voltra_scene::particles::{Flipbook, FlipbookTiming};
use voltra_scene::ParticleEmitter;

use super::super::active;
use super::super::path_field::{self, PathEdit};

const EDIT: &str = "Edit flipbook";

pub(super) fn show(
    ui: &mut Ui,
    emitter: &mut ParticleEmitter,
    atlases: &mut Atlases,
) -> Option<&'static str> {
    ui.label("flipbook atlas")
        .on_hover_text("frames of a *.atlas.ron each particle steps through, cut from the texture");
    let committed = emitter
        .flipbook
        .as_ref()
        .map(|flipbook| flipbook.atlas.as_str().to_owned())
        .unwrap_or_default();
    let mut claim = None;
    if let Some(edit) = path_field::show(
        ui,
        "particle_flipbook",
        "path/to/sheet.atlas.ron",
        &committed,
    ) {
        emitter.flipbook = match edit {
            PathEdit::Cleared => None,
            PathEdit::Set(path) => Some(match emitter.flipbook.take() {
                Some(flipbook) => Flipbook {
                    atlas: path,
                    ..flipbook
                },
                None => Flipbook::new(path),
            }),
        };
        emitter.resolve_atlas(atlases);
        claim = Some(EDIT);
    }
    let Some(flipbook) = &mut emitter.flipbook else {
        return claim;
    };
    let in_atlas = flipbook
        .atlas_handle
        .and_then(|handle| atlases.try_get(handle))
        .map_or(0, |atlas| atlas.len());
    let playing = flipbook.uvs(Some(atlases)).len();
    ui.label(RichText::new(format!("plays {playing} of the atlas's {in_atlas} frames")).weak());
    claim = claim.or(frames(ui, flipbook));
    ui.horizontal(|ui| {
        let over_life = matches!(flipbook.timing, FlipbookTiming::OverLife { .. });
        egui::ComboBox::from_id_salt("flipbook_timing")
            .selected_text(if over_life {
                "times per life"
            } else {
                "frames / s"
            })
            .show_ui(ui, |ui| {
                if ui.selectable_label(over_life, "times per life").clicked() && !over_life {
                    flipbook.timing = FlipbookTiming::OverLife { cycles: 1.0 };
                    claim = Some(EDIT);
                }
                if ui.selectable_label(!over_life, "frames / s").clicked() && over_life {
                    flipbook.timing = FlipbookTiming::PerSecond { fps: 12.0 };
                    claim = Some(EDIT);
                }
            });
        let rate = match &mut flipbook.timing {
            FlipbookTiming::OverLife { cycles } => cycles,
            FlipbookTiming::PerSecond { fps } => fps,
        };
        claim = claim.or(active(
            &ui.add(DragValue::new(rate).speed(0.1).range(0.0..=240.0)),
            EDIT,
        ));
    });
    claim
}

/// The frame list as text — `0 1 2 5`, empty for all of them — committed
/// when the box is left. A word that is not a frame index is refused and
/// the box goes back to what was stored.
fn frames(ui: &mut Ui, flipbook: &mut Flipbook) -> Option<&'static str> {
    let stored = flipbook
        .frames
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    let id = ui.id().with("flipbook_frames");
    let mut text = ui
        .ctx()
        .data_mut(|d| d.get_temp_mut_or_insert_with(id, || stored.clone()).clone());
    let mut claim = None;
    ui.horizontal(|ui| {
        ui.label("frames");
        let response = ui.add(TextEdit::singleline(&mut text).hint_text("all, in order"));
        if response.lost_focus() && text != stored {
            match parse_frames(&text) {
                Some(frames) => {
                    flipbook.frames = frames;
                    claim = Some(EDIT);
                }
                None => {
                    log::error!("not a list of frame indices: {text:?}");
                    text = stored.clone();
                }
            }
        }
    });
    ui.ctx().data_mut(|d| {
        d.insert_temp(id, text);
    });
    claim
}

/// Frame indices separated by spaces or commas; `None` if a word is not one.
fn parse_frames(text: &str) -> Option<Vec<u32>> {
    text.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|word| !word.is_empty())
        .map(|word| word.parse().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_list_reads_spaces_and_commas_and_refuses_words() {
        assert_eq!(parse_frames("0 1,2  5"), Some(vec![0, 1, 2, 5]));
        assert_eq!(parse_frames(""), Some(vec![]));
        assert_eq!(parse_frames("0 one"), None);
    }
}
