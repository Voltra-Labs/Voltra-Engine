//! The components only some of the selection have: listed, greyed, not
//! editable, with a way to give one to the rest.
//!
//! Unity hides such a component from a multi-object inspector with a
//! note that it is not shared; listing each with its count says which,
//! and "add to the rest" makes it shared, after which it is edited like
//! any other.

use voltra_core::egui::{RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::{hierarchy, ComponentRegistry};

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
/// it did. A transform is refused, and said so, where it would be a second
/// one or cross dimensions with a parent or child
/// ([`hierarchy::can_take_transform`]).
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
    // A transform is not given to one that carries the other dimension's,
    // or that sits in a chain of the other dimension: an entity is 2D or 3D.
    let dimension = hierarchy::Dimension::of_transform_name(name);
    let lacking: Vec<Entity> = entities
        .into_iter()
        .filter(|&e| registry.has(world, e, name) == Some(false))
        .filter(|&e| match dimension {
            Some(dimension) => match hierarchy::can_take_transform(world, e, dimension) {
                Ok(()) => true,
                Err(refused) => {
                    log::warn!("{name} not added to {e:?}: {refused}");
                    false
                }
            },
            None => true,
        })
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
    use voltra_scene::{Collider2D, Sprite2D, Transform2D, Transform3D};

    #[test]
    fn a_component_is_added_only_where_it_is_missing() {
        let registry = ComponentRegistry::with_defaults();
        let mut world = World::new();
        let with = world.spawn();
        let without = world.spawn();
        let shaped = Collider2D::Circle { radius: 4.0 };
        world.insert(with, shaped);
        world.insert(without, Sprite2D::default());

        let added = add_where_missing(&mut world, &registry, [with, without], "Collider2D");

        assert_eq!(added, [without]);
        assert_eq!(world.get::<Collider2D>(with), Some(&shaped), "kept its own");
        assert_eq!(
            world.get::<Collider2D>(without),
            Some(&Collider2D::default())
        );
    }

    #[test]
    fn a_second_transform_is_not_added() {
        let registry = ComponentRegistry::with_defaults();
        let mut world = World::new();
        let flat = world.spawn();
        let deep = world.spawn();
        world.insert(flat, Transform2D::default());
        world.insert(deep, Transform3D::default());

        let added = add_where_missing(&mut world, &registry, [flat, deep], Transform3D::NAME);

        assert!(added.is_empty(), "the 2D one refused, the 3D one has it");
        assert!(world.get::<Transform3D>(flat).is_none());
    }
}
