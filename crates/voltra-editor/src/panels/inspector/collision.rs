//! What a collider touches, and whether it stops anything.

use voltra_core::egui::{RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::{CollisionLayers, Sensor};

/// The sensor mark and the two layer masks, for an entity that has a collider.
///
/// Takes the world rather than a `&mut` component, because both of these are
/// absent by default and the first edit is what writes them down. Absent means
/// "everything, and solid", so the widget shows the defaults and only inserts
/// a component once something is actually changed — an entity nobody has
/// filtered keeps a scene file free of a filter that says nothing.
pub(super) fn show(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    ui.label(RichText::new("Collision").strong());

    let mut claim = None;

    let mut sensor = world.get::<Sensor>(entity).is_some();
    // A checkbox is one click rather than a held interaction, so `active` —
    // which asks about dragging and focus — would never report it, exactly as
    // in the camera's widget.
    if ui
        .checkbox(&mut sensor, "sensor")
        .on_hover_text("detected and reported, never solved: nothing stands on it")
        .changed()
    {
        if sensor {
            world.insert(entity, Sensor);
        } else {
            world.remove::<Sensor>(entity);
        }
        claim = Some("Toggle sensor");
    }

    let mut layers = world
        .get::<CollisionLayers>(entity)
        .copied()
        .unwrap_or_default();

    ui.label("on")
        .on_hover_text("the layers this collider is on");
    let mut changed = bits(ui, "layers", &mut layers.layers);
    ui.label("sees")
        .on_hover_text("the layers it looks at — both sides must look at the other");
    changed |= bits(ui, "mask", &mut layers.mask);

    if changed {
        world.insert(entity, layers);
        claim = claim.or(Some("Set collision layers"));
    }

    claim
}

/// One toggle per layer, wrapped to the panel's width.
///
/// Every layer, not the first eight: the mask is thirty-two bits wide and a
/// widget that shows fewer would make the rest unreachable from the editor
/// while still loading from a scene file.
fn bits(ui: &mut Ui, id: &str, mask: &mut u32) -> bool {
    let mut changed = false;
    // Salted, or the two rows of toggles collide on ids and dragging across
    // one row would light up the other.
    ui.push_id(id, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for index in 0..CollisionLayers::COUNT {
                let bit = CollisionLayers::bit(index);
                let mut on = *mask & bit != 0;
                if ui
                    .toggle_value(&mut on, RichText::new(index.to_string()).small())
                    .on_hover_text(format!("layer {index}"))
                    .changed()
                {
                    *mask ^= bit;
                    changed = true;
                }
            }
        });
    });
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_core::egui::Context;
    use voltra_scene::Collider;

    #[test]
    fn drawing_the_widget_writes_nothing_by_itself() {
        // The promise the defaults rest on: a collider nobody has filtered
        // must stay a collider with no filter, or every scene saved after
        // opening the inspector gains a component that says nothing.
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Collider::Circle { radius: 1.0 });
        let mut claim = Some("unset");

        let ctx = Context::default();
        let _ = ctx.run_ui(Default::default(), |ui| {
            claim = show(ui, &mut world, entity);
        });

        assert_eq!(claim, None, "nothing was interacted with");
        assert_eq!(world.get::<CollisionLayers>(entity), None);
        assert_eq!(world.get::<Sensor>(entity), None);
    }
}
