//! Whether a server remembers an entity's collider for rewound hit tests.

use voltra_core::egui::Ui;
use voltra_ecs::{Entity, World};
use voltra_scene::{Collider, LagCompensated};

/// The lag-compensated mark, for a replicated entity or one that has it.
///
/// Takes the world, not a component: absent is "not remembered", and the
/// checkbox inserts or removes the marker, as the sensor's does.
pub(super) fn show(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    let mut marked = world.get::<LagCompensated>(entity).is_some();
    let hover = if world.get::<Collider>(entity).is_some() {
        "the server remembers where this collider was each tick, so a client's shot \
         is tested where that client saw it"
    } else {
        "the server remembers where this entity's collider was each tick — it has none \
         yet, so there is nothing to remember"
    };
    if ui
        .checkbox(&mut marked, "lag compensated")
        .on_hover_text(hover)
        .changed()
    {
        if marked {
            world.insert(entity, LagCompensated);
        } else {
            world.remove::<LagCompensated>(entity);
        }
        return Some("Toggle lag compensation");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_core::egui;

    #[test]
    fn nothing_is_written_until_it_is_ticked() {
        let mut world = World::new();
        let entity = world.spawn();
        let ctx = egui::Context::default();
        let mut claim = Some("untouched");
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            claim = show(ui, &mut world, entity);
        });
        assert_eq!(claim, None);
        assert_eq!(world.get::<LagCompensated>(entity), None);
    }
}
