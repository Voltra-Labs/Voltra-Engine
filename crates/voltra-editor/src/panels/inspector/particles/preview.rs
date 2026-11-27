//! The preview's controls — restart, a burst — and the presets an emitter
//! starts from.
//!
//! Restart and burst touch only the live particles, which a scene never
//! saves, so neither is an edit; a preset rewrites the emitter and is one
//! undo entry.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::particles::presets::Preset;
use voltra_scene::ParticleEmitter;

/// The controls, and a preset picked — for the caller to apply to every
/// selected emitter from its own, which keeps each one's texture, flipbook
/// and layers.
pub(super) fn show(ui: &mut Ui, emitter: &mut ParticleEmitter) -> Option<Preset> {
    let mut picked = None;
    let count_id = ui.id().with("burst_count");
    let mut count: u32 = ui.ctx().data(|d| d.get_temp(count_id)).unwrap_or(20);
    ui.horizontal(|ui| {
        if ui
            .button("Restart")
            .on_hover_text("clear the particles and start again")
            .clicked()
        {
            emitter.restart();
        }
        if ui
            .button("Burst")
            .on_hover_text("emit this many at once")
            .clicked()
        {
            emitter.burst(count);
        }
        ui.add(DragValue::new(&mut count).range(1..=10_000));
        egui::ComboBox::from_id_salt("particle_preset")
            .selected_text("preset…")
            .show_ui(ui, |ui| {
                for preset in Preset::ALL {
                    if ui.selectable_label(false, preset.label()).clicked() {
                        picked = Some(preset);
                    }
                }
            })
            .response
            .on_hover_text("start from smoke, sparks or rain; the texture and layers stay");
    });
    ui.ctx().data_mut(|d| {
        d.insert_temp(count_id, count);
    });
    ui.label(RichText::new("previews while selected, without playing").weak());
    picked
}
