//! Which named sorting layer a drawable is on.

use voltra_core::egui::{self, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::{ParticleEmitter, SortingLayers, Sprite, Text, Tilemap};

/// The dropdown for each drawable `entity` carries — a sprite, a tilemap
/// layer, a text, an emitter — salted by which it is.
pub(super) fn show_for(
    ui: &mut Ui,
    world: &mut World,
    entity: Entity,
    layers: &SortingLayers,
) -> Option<&'static str> {
    let mut claim = None;
    if let Some(sprite) = world.get_mut::<Sprite>(entity) {
        claim = claim.or(show(ui, "sprite", &mut sprite.sorting_layer, layers));
    }
    if let Some(map) = world.get_mut::<Tilemap>(entity) {
        claim = claim.or(show(ui, "tilemap", &mut map.sorting_layer, layers));
    }
    if let Some(text) = world.get_mut::<Text>(entity) {
        claim = claim.or(show(ui, "text", &mut text.sorting_layer, layers));
    }
    if let Some(emitter) = world.get_mut::<ParticleEmitter>(entity) {
        claim = claim.or(show(ui, "particles", &mut emitter.sorting_layer, layers));
    }
    if let Some(line) = world.get_mut::<voltra_scene::Line2D>(entity) {
        claim = claim.or(show(ui, "line", &mut line.sorting_layer, layers));
    }
    if let Some(polygon) = world.get_mut::<voltra_scene::Polygon2D>(entity) {
        claim = claim.or(show(ui, "polygon", &mut polygon.sorting_layer, layers));
    }
    if let Some(trail) = world.get_mut::<voltra_scene::Trail2D>(entity) {
        claim = claim.or(show(ui, "trail", &mut trail.sorting_layer, layers));
    }
    claim
}

/// A dropdown of the project's layers, back to front. A name not in the list
/// — a scene from a project with other layers — is shown as it is and kept
/// until another is picked.
pub(super) fn show(
    ui: &mut Ui,
    salt: &str,
    layer: &mut String,
    layers: &SortingLayers,
) -> Option<&'static str> {
    let mut claim = None;
    ui.horizontal(|ui| {
        ui.label("sorting layer")
            .on_hover_text("drawn after every earlier layer, whatever the sort order");
        egui::ComboBox::from_id_salt(("sorting_layer", salt))
            .selected_text(layer.as_str())
            .show_ui(ui, |ui| {
                for name in layers.names() {
                    if ui.selectable_label(layer == name, name.as_str()).clicked() && layer != name
                    {
                        *layer = name.clone();
                        claim = Some("Set sorting layer");
                    }
                }
            });
    });
    claim
}
