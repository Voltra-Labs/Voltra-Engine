//! The machine's transitions, each with the seconds it crossfades over.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::StateMachine;

/// One row per transition — where from, where to, and its fade. Answers
/// whether a fade was edited.
pub(super) fn show(ui: &mut Ui, machine: &mut StateMachine) -> bool {
    let mut edited = false;
    egui::CollapsingHeader::new("transitions")
        .id_salt("animator_transitions")
        .show(ui, |ui| {
            ui.label(
                RichText::new("seconds each crossfades over; 0 cuts")
                    .weak()
                    .small(),
            );
            egui::Grid::new("animator_transitions_grid")
                .num_columns(2)
                .show(ui, |ui| {
                    for transition in &mut machine.transitions {
                        let from = transition.from.as_deref().unwrap_or("any");
                        ui.label(format!("{from} to {}", transition.to));
                        let fade = ui.add(
                            // No upper bound: a range clamps the value it is
                            // shown, and a long fade typed in the file would be
                            // cut down (and marked edited) by opening this.
                            DragValue::new(&mut transition.duration)
                                .speed(0.01)
                                .range(0.0..=f32::INFINITY)
                                .suffix("s"),
                        );
                        edited |= fade.changed();
                        ui.end_row();
                    }
                });
        });
    edited
}
