//! The audio source's clip, levels, flags and bus, and a button to hear it.

use voltra_assets::{AssetPath, Clips};
use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_core::{Audio, VoiceId};
use voltra_scene::AudioSource;

use super::active;
use super::mixed;
use super::path_field::{self, PathEdit};
use voltra_scene::format::serial_tree::Step;

/// What the bus box shows for a source on the master, whose stored name is
/// empty so the file stays as it was before buses existed.
const MASTER: &str = "master";

pub(super) fn show(
    ui: &mut Ui,
    source: &mut AudioSource,
    clips: &mut Clips,
    audio: &mut Audio,
    audition: &mut Option<VoiceId>,
) -> Option<&'static str> {
    ui.label(RichText::new("Audio source").strong());

    let mut claim = clip_ui(ui, source, clips);

    egui::Grid::new("audio_source")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("volume");
            claim = claim.or(active(
                &ui.add(
                    DragValue::new(&mut source.volume)
                        .speed(0.01)
                        .range(0.0..=4.0),
                ),
                "Set volume",
            ));
            ui.end_row();

            ui.label("pitch");
            // Floored above zero: zero holds the first frame forever, which is
            // a real setting for a game to choose and a confusing one to land
            // on by dragging.
            claim = claim.or(active(
                &ui.add(
                    DragValue::new(&mut source.pitch)
                        .speed(0.01)
                        .range(0.01..=4.0),
                ),
                "Set pitch",
            ));
            ui.end_row();

            ui.label("range");
            claim = claim
                .or(active(
                    &ui.add(
                        DragValue::new(&mut source.range)
                            .speed(0.1)
                            .range(0.0..=f32::MAX),
                    ),
                    "Set range",
                ))
                .or(range_hint(ui, source.range));
            ui.end_row();

            ui.label("bus");
            claim = claim.or(bus_ui(ui, source, audio));
            ui.end_row();

            ui.label("");
            if ui.checkbox(&mut source.looping, "loop").changed() {
                claim = claim.or(Some("Set loop"));
            }
            ui.end_row();

            ui.label("");
            if ui
                .checkbox(&mut source.play_on_spawn, "play on spawn")
                .changed()
            {
                claim = claim.or(Some("Set play on spawn"));
            }
            ui.end_row();
        });

    audition_ui(ui, source, clips, audio, audition);
    claim
}

/// Says what a zero range means, which the number alone does not.
///
/// Returns no claim: it is a label, and only an edit opens an undo entry.
fn range_hint(ui: &mut Ui, range: f32) -> Option<&'static str> {
    if range <= 0.0 {
        ui.label(RichText::new("everywhere").weak());
    }
    None
}

/// The clip path editor.
fn clip_ui(ui: &mut Ui, source: &mut AudioSource, clips: &mut Clips) -> Option<&'static str> {
    let committed = source
        .clip
        .as_ref()
        .map(AssetPath::as_str)
        .unwrap_or("")
        .to_owned();
    let path = match path_field::show(ui, "clip_path", "path/to/sound.wav", &committed)? {
        PathEdit::Cleared => None,
        PathEdit::Set(path) => Some(path),
    };
    source.set_clip(path, clips);
    Some("Set clip")
}

/// The bus picker: every bus the game's table holds.
///
/// A list rather than a text box, because a misspelled bus is not an error
/// anywhere — it becomes a bus of its own that no slider reaches — and a list
/// is what stops one being typed. A name the scene file brought that the
/// table has not seen is registered by being shown, so it is in the list too.
fn bus_ui(ui: &mut Ui, source: &mut AudioSource, audio: &mut Audio) -> Option<&'static str> {
    audio.bus(&source.bus);
    let shown = if source.bus.is_empty() {
        MASTER.to_owned()
    } else {
        source.bus.clone()
    };

    let mut picked = None;
    egui::ComboBox::from_id_salt("bus")
        .selected_text(&shown)
        .show_ui(ui, |ui| {
            for name in audio.buses().names() {
                let clicked = ui.selectable_label(name == shown, name).clicked();
                if clicked {
                    mixed::touch(ui, "AudioSource", vec![Step::Field("bus")]);
                }
                if clicked && name != shown {
                    picked = Some(name.to_owned());
                }
            }
        });

    let picked = picked?;
    source.bus = if picked == MASTER {
        String::new()
    } else {
        picked
    };
    Some("Set bus")
}

/// Play and Stop for the clip as the source would play it, minus position.
///
/// A button, not a checkbox, because an audition is an action and not a
/// setting: nothing about it is saved, and the scene is no different after
/// it. Unity's inspector preview is the same shape. Played flat — centred, at
/// the source's own volume — because where the listener is depends on the
/// running game, and on its bus, so the preview is as loud as the game's mix
/// will make it.
///
/// One audition at a time: a second Play stops the first rather than layering
/// two takes of one sound, which is never what pressing it twice means.
fn audition_ui(
    ui: &mut Ui,
    source: &AudioSource,
    clips: &Clips,
    audio: &mut Audio,
    audition: &mut Option<VoiceId>,
) {
    let clip = source.clip_handle.and_then(|handle| clips.try_get(handle));

    ui.horizontal(|ui| {
        let play = ui.add_enabled(clip.is_some(), egui::Button::new("Play"));
        let stop = ui.add_enabled(audition.is_some(), egui::Button::new("Stop"));

        if stop.clicked() {
            if let Some(voice) = audition.take() {
                audio.stop(voice);
            }
        }
        if let (true, Some(clip)) = (play.clicked(), clip) {
            if let Some(voice) = audition.take() {
                audio.stop(voice);
            }
            let params = source.params(audio.buses_mut());
            *audition = Some(audio.play(clip, params));
        }

        if audio.is_silent() {
            ui.label(RichText::new("no output device").weak());
        }
    });
}
