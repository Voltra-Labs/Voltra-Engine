//! How an entity is sent to the clients of a session: its priority in a
//! client's budget, and its part in interest management.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::Replicated;

use super::active;

/// Its priority, whether every client is sent it, and its own relevance
/// radius when it has one.
pub(super) fn show(ui: &mut Ui, replicated: &mut Replicated) -> Option<&'static str> {
    ui.label(RichText::new("Replicated").strong())
        .on_hover_text("the server sends this entity's replicated components to its clients");
    let mut claim = None;
    egui::Grid::new("replicated").num_columns(2).show(ui, |ui| {
        ui.label("priority").on_hover_text(
            "its share of a client's snapshot budget next to the others': banked while it waits",
        );
        claim = claim.or(active(
            &ui.add(
                DragValue::new(&mut replicated.priority)
                    .speed(0.05)
                    .range(0.0..=100.0),
            ),
            "Set replication",
        ));
        ui.end_row();

        ui.label("always relevant").on_hover_text(
            "with interest management, sent to every client wherever it is: \
             the match's state, an objective",
        );
        if ui.checkbox(&mut replicated.always_relevant, "").changed() {
            claim = claim.or(Some("Set replication"));
        }
        ui.end_row();

        ui.label("own radius").on_hover_text(
            "with interest management, relevant within this distance instead of the session's",
        );
        let mut own = replicated.relevance_radius.is_some();
        if ui.checkbox(&mut own, "").changed() {
            replicated.relevance_radius = own.then_some(30.0);
            claim = claim.or(Some("Set replication"));
        }
        ui.end_row();
        if let Some(radius) = replicated.relevance_radius.as_mut() {
            ui.label("radius");
            claim = claim.or(active(
                &ui.add(
                    DragValue::new(radius)
                        .speed(0.5)
                        .range(0.01..=100_000.0)
                        .suffix(" units"),
                ),
                "Set replication",
            ));
            ui.end_row();
        }
    });
    claim
}
