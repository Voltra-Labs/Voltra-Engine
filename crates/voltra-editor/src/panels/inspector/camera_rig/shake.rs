//! The shake section: how it shakes, and a button to try it.

use voltra_core::egui::{self, RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::CameraShake;

use super::{number, pair};

pub(super) fn shake(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    ui.label(RichText::new("Shake").strong())
        .on_hover_text("shaken by trauma a game adds: add_trauma");
    let rig = world
        .get_mut::<CameraShake>(entity)
        .expect("tested by the caller");
    let mut claim = None;
    egui::Grid::new("camera_shake")
        .num_columns(2)
        .show(ui, |ui| {
            claim = claim.or(pair(
                ui,
                "max offset",
                &mut rig.max_offset,
                "how far it moves at full trauma",
            ));
            claim = claim.or(number(
                ui,
                "frequency",
                &mut rig.frequency,
                "roughly shakes a second",
            ));
            claim = claim.or(number(
                ui,
                "decay",
                &mut rig.decay,
                "trauma lost per second",
            ));
        });
    // Seen only while the game runs, which is when trauma does anything.
    if ui
        .button("Test shake")
        .on_hover_text("adds half a trauma; shakes while playing")
        .clicked()
    {
        rig.add_trauma(0.5);
    }
    claim
}
