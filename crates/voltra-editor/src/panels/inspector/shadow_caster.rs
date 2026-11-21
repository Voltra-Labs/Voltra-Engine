//! Whether an entity blocks the light of lights that cast shadows.

use voltra_core::egui::{RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::{CasterShape, ShadowCaster2D};

/// The caster toggle and its settings, for an entity that has a collider, is
/// a solid layer, or already casts.
///
/// Takes the world rather than a `&mut` component, as the collision filter
/// does: the caster is absent by default, and ticking the box is what adds
/// it — shaped by the collider, which is what an entity reaching this widget
/// through its collider would mean. A polygon of its own is written in the
/// scene file for now; it is shown and kept here, and the viewport draws
/// the selected caster's outline (`view::caster`).
pub(super) fn show(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    ui.label(RichText::new("Shadows").strong())
        .on_hover_text("blocks the light of lights that cast shadows");
    let mut claim = None;

    let mut casts = world.get::<ShadowCaster2D>(entity).is_some();
    // A checkbox is one click, so `changed` is the whole edit.
    if ui.checkbox(&mut casts, "casts shadows").changed() {
        if casts {
            world.insert(entity, ShadowCaster2D::default());
        } else {
            world.remove::<ShadowCaster2D>(entity);
        }
        claim = Some("Toggle shadow caster");
    }
    let Some(caster) = world.get_mut::<ShadowCaster2D>(entity) else {
        return claim;
    };
    if ui
        .checkbox(&mut caster.self_shadow, "self shadow")
        .on_hover_text("shadowed by its own near side; off, light reaches it")
        .changed()
    {
        claim = claim.or(Some("Toggle self shadow"));
    }
    match &mut caster.shape {
        CasterShape::Collider => {
            ui.label(RichText::new("shaped by its collider").weak());
        }
        CasterShape::Polygon { points, closed } => {
            ui.label(RichText::new(format!("own polygon, {} points", points.len())).weak());
            if ui
                .checkbox(closed, "closed")
                .on_hover_text("an outline; open, a line that blocks both sides")
                .changed()
            {
                claim = claim.or(Some("Toggle closed outline"));
            }
        }
    }
    claim
}
