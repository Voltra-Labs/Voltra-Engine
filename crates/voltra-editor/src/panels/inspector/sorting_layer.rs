//! Which named sorting layer a drawable is on.

use voltra_core::egui::{self, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::format::serial_tree::Step;
use voltra_scene::{ParticleEmitter, SortingLayers, Sprite2D, Text, Tilemap};

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
    if let Some(sprite) = world
        .get_mut::<Sprite2D>(entity)
        .filter(|_| open(Sprite2D::NAME))
    {
        claim = claim.or(show(ui, Sprite2D::NAME, &mut sprite.sorting_layer, layers));
    }
    if let Some(map) = world
        .get_mut::<Tilemap>(entity)
        .filter(|_| open(Tilemap::NAME))
    {
        claim = claim.or(show(ui, Tilemap::NAME, &mut map.sorting_layer, layers));
    }
    if let Some(text) = world.get_mut::<Text>(entity).filter(|_| open(Text::NAME)) {
        claim = claim.or(show(ui, Text::NAME, &mut text.sorting_layer, layers));
    }
    if let Some(emitter) = world
        .get_mut::<ParticleEmitter>(entity)
        .filter(|_| open(ParticleEmitter::NAME))
    {
        claim = claim.or(show(
            ui,
            ParticleEmitter::NAME,
            &mut emitter.sorting_layer,
            layers,
        ));
    }
    if let Some(line) = world
        .get_mut::<voltra_scene::Line2D>(entity)
        .filter(|_| open(voltra_scene::Line2D::NAME))
    {
        claim = claim.or(show(
            ui,
            voltra_scene::Line2D::NAME,
            &mut line.sorting_layer,
            layers,
        ));
    }
    if let Some(polygon) = world
        .get_mut::<voltra_scene::Polygon2D>(entity)
        .filter(|_| open(voltra_scene::Polygon2D::NAME))
    {
        claim = claim.or(show(
            ui,
            voltra_scene::Polygon2D::NAME,
            &mut polygon.sorting_layer,
            layers,
        ));
    }
    if let Some(trail) = world
        .get_mut::<voltra_scene::Trail2D>(entity)
        .filter(|_| open(voltra_scene::Trail2D::NAME))
    {
        claim = claim.or(show(
            ui,
            voltra_scene::Trail2D::NAME,
            &mut trail.sorting_layer,
            layers,
        ));
    }
    if let Some(skinned) = world
        .get_mut::<voltra_scene::SkinnedSprite>(entity)
        .filter(|_| open(voltra_scene::SkinnedSprite::NAME))
    {
        claim = claim.or(show(
            ui,
            voltra_scene::SkinnedSprite::NAME,
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
