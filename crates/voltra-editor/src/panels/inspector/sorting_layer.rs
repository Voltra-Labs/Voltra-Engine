//! Which named sorting layer a drawable is on.

use voltra_core::egui::{self, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::format::serial_tree::Step;
use voltra_scene::{ParticleEmitter, SortingLayers, Sprite, Text, Tilemap};

use super::mixed;
use crate::multi_edit::MultiEdit;

/// The dropdown for each drawable `entity` carries — a sprite, a tilemap
/// layer, a text, an emitter — salted by which it is, its registered name. Not for a drawable
/// only some of the selection have: its section is not drawn either.
pub(super) fn show_for(
    ui: &mut Ui,
    world: &mut World,
    entity: Entity,
    layers: &SortingLayers,
    multi: &MultiEdit,
) -> Option<&'static str> {
    let mut claim = None;
    let open = |name: &str| multi.editable(&[name]);
    if let Some(sprite) = world.get_mut::<Sprite>(entity).filter(|_| open("Sprite")) {
        claim = claim.or(show(ui, "Sprite", &mut sprite.sorting_layer, layers));
    }
    if let Some(map) = world.get_mut::<Tilemap>(entity).filter(|_| open("Tilemap")) {
        claim = claim.or(show(ui, "Tilemap", &mut map.sorting_layer, layers));
    }
    if let Some(text) = world.get_mut::<Text>(entity).filter(|_| open("Text")) {
        claim = claim.or(show(ui, "Text", &mut text.sorting_layer, layers));
    }
    if let Some(emitter) = world
        .get_mut::<ParticleEmitter>(entity)
        .filter(|_| open("ParticleEmitter"))
    {
        claim = claim.or(show(
            ui,
            "ParticleEmitter",
            &mut emitter.sorting_layer,
            layers,
        ));
    }
    if let Some(line) = world
        .get_mut::<voltra_scene::Line2D>(entity)
        .filter(|_| open("Line2D"))
    {
        claim = claim.or(show(ui, "Line2D", &mut line.sorting_layer, layers));
    }
    if let Some(polygon) = world
        .get_mut::<voltra_scene::Polygon2D>(entity)
        .filter(|_| open("Polygon2D"))
    {
        claim = claim.or(show(ui, "Polygon2D", &mut polygon.sorting_layer, layers));
    }
    if let Some(trail) = world
        .get_mut::<voltra_scene::Trail2D>(entity)
        .filter(|_| open("Trail2D"))
    {
        claim = claim.or(show(ui, "Trail2D", &mut trail.sorting_layer, layers));
    }
    if let Some(skinned) = world
        .get_mut::<voltra_scene::SkinnedSprite>(entity)
        .filter(|_| open("SkinnedSprite"))
    {
        claim = claim.or(show(
            ui,
            "SkinnedSprite",
            &mut skinned.sorting_layer,
            layers,
        ));
    }
    claim
}

/// A dropdown of the project's layers, back to front. A name not in the list
/// — a scene from a project with other layers — is shown as it is and kept
/// until another is picked.
pub(super) fn show(
    ui: &mut Ui,
    component: &'static str,
    layer: &mut String,
    layers: &SortingLayers,
) -> Option<&'static str> {
    let mut claim = None;
    ui.horizontal(|ui| {
        ui.label("sorting layer")
            .on_hover_text("drawn after every earlier layer, whatever the sort order");
        egui::ComboBox::from_id_salt(("sorting_layer", component))
            .selected_text(layer.as_str())
            .show_ui(ui, |ui| {
                for name in layers.names() {
                    let response = ui.selectable_label(layer == name, name.as_str());
                    let path = vec![Step::Field("sorting_layer")];
                    if mixed::chosen(ui, response, component, path).clicked() && layer != name {
                        *layer = name.clone();
                        claim = Some("Set sorting layer");
                    }
                }
            });
    });
    claim
}
