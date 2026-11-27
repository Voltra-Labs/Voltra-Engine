//! The components only some of the selection have: listed, greyed, not
//! editable, with a way to give one to the rest.
//!
//! Unity hides such a component from a multi-object inspector with a
//! note that it is not shared; listing each with its count says which,
//! and "add to the rest" makes it shared, after which it is edited like
//! any other.

use voltra_core::egui::{RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::ComponentRegistry;

use crate::multi_edit::MultiEdit;

/// The list, and the entities a button gave a component to.
pub(super) fn show(
    ui: &mut Ui,
    world: &mut World,
    registry: &ComponentRegistry,
    multi: &MultiEdit,
) -> Vec<Entity> {
    let mut added = Vec::new();
    if multi.partial().is_empty() {
        return added;
    }
    ui.label(RichText::new("Not on all of them").weak())
        .on_hover_text("edited one entity at a time, or added to the rest first");
    for &(name, held) in multi.partial() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{name} ({held} of {})", multi.len())).weak());
            if registry.template(name).is_some()
                && ui
                    .small_button("add to the rest")
                    .on_hover_text(format!("gives a default {name} to each that has none"))
                    .clicked()
            {
                added = add_where_missing(world, registry, multi.all(), name);
            }
        });
    }
    ui.separator();
    added
}

/// Puts `name`'s template on each of `entities` without one; the entities
/// it did.
pub(super) fn add_where_missing(
    world: &mut World,
    registry: &ComponentRegistry,
    entities: impl IntoIterator<Item = Entity>,
    name: &str,
) -> Vec<Entity> {
    let template = match registry.template(name) {
        Some(Ok(template)) => template,
        Some(Err(error)) => {
            log::error!("cannot add {name}: {error}");
            return Vec::new();
        }
        None => return Vec::new(),
    };
    let lacking: Vec<Entity> = entities
        .into_iter()
        .filter(|&e| registry.has(world, e, name) == Some(false))
        .collect();
    lacking
        .into_iter()
        .filter(
            |&entity| match registry.load_one(world, entity, name, &template) {
                Some(Ok(())) => true,
                Some(Err(error)) => {
                    log::error!("cannot add {name}: {error}");
                    false
                }
                None => false,
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_scene::{Collider, Sprite};

    #[test]
    fn a_component_is_added_only_where_it_is_missing() {
        let registry = ComponentRegistry::with_defaults();
        let mut world = World::new();
        let with = world.spawn();
        let without = world.spawn();
        let shaped = Collider::Circle { radius: 4.0 };
        world.insert(with, shaped);
        world.insert(without, Sprite::default());

        let added = add_where_missing(&mut world, &registry, [with, without], "Collider");

        assert_eq!(added, [without]);
        assert_eq!(world.get::<Collider>(with), Some(&shaped), "kept its own");
        assert_eq!(world.get::<Collider>(without), Some(&Collider::default()));
    }
}
