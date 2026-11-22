//! Registering, saving, loading and removing components by name.

use super::*;
use crate::{Sprite, Transform};
use voltra_render::glam::Vec2;

fn world_with_one_sprite() -> (World, Entity) {
    let mut world = World::new();
    let e = world.spawn();
    world.insert(e, Transform::from_translation(Vec2::new(1.0, 2.0)));
    world.insert(e, Sprite::default().with_sort_order(3));
    (world, e)
}

#[test]
fn the_defaults_cover_the_built_in_components() {
    let registry = ComponentRegistry::with_defaults();
    let names: Vec<_> = registry.names().collect();
    assert!(names.contains(&"Transform"), "got {names:?}");
    assert!(names.contains(&"Sprite"), "got {names:?}");
}

#[test]
fn an_unregistered_name_is_not_an_error() {
    // The caller has to be able to tell "I do not know this" from "I know it
    // and it broke", because the first is preserved and the second is not.
    let (world, entity) = world_with_one_sprite();
    let registry = ComponentRegistry::with_defaults();
    assert!(registry.save_one(&world, entity, "Physics").is_none());
}

#[test]
fn a_registered_component_round_trips_through_a_value() {
    let (world, entity) = world_with_one_sprite();
    let registry = ComponentRegistry::with_defaults();

    let value = registry
        .save_one(&world, entity, "Sprite")
        .expect("Sprite is registered")
        .expect("saving a valid Sprite cannot fail");

    let mut target = World::new();
    let copy = target.spawn();
    registry
        .load_one(&mut target, copy, "Sprite", &value)
        .expect("Sprite is registered")
        .expect("the value came from a Sprite, so it must load as one");

    assert_eq!(target.get::<Sprite>(copy), world.get::<Sprite>(entity));
}

#[test]
fn load_one_is_also_none_for_an_unregistered_name() {
    // The given suite only checked the "unregistered -> None" side on
    // `save_one` (see `an_unregistered_name_is_not_an_error`) and the
    // "registered and broken -> Some(Err)" side on `load_one` (see
    // `a_registered_component_with_wrong_data_is_an_error`). Neither calls
    // `load_one` with an unregistered name, so an implementation that
    // collapsed "unknown" into "broken" on the load side alone would still
    // pass every other test here. This is the missing corner.
    let mut world = World::new();
    let entity = world.spawn();
    let registry = ComponentRegistry::with_defaults();
    let value = RawValue::from_ron("()").expect("valid RON");
    assert!(registry
        .load_one(&mut world, entity, "Physics", value)
        .is_none());
}

#[test]
fn an_entity_without_the_component_saves_nothing() {
    // Registered, but this entity does not have one. Distinct from both
    // "unknown name" and "failed" — the map simply has no entry.
    let mut world = World::new();
    let bare = world.spawn();
    let registry = ComponentRegistry::with_defaults();
    assert!(registry.save_one(&world, bare, "Sprite").is_none());
}

#[test]
fn a_registered_component_with_wrong_data_is_an_error() {
    let mut world = World::new();
    let entity = world.spawn();
    let registry = ComponentRegistry::with_defaults();

    // A string where a Sprite struct belongs.
    let nonsense = RawValue::from_ron("\"not a sprite\"").expect("valid RON");

    let result = registry
        .load_one(&mut world, entity, "Sprite", nonsense)
        .expect("Sprite is registered");
    assert!(
        matches!(result, Err(SceneError::Component { .. })),
        "expected a Component error, got {result:?}"
    );
}

#[test]
fn remove_one_takes_the_component_off() {
    let (mut world, entity) = world_with_one_sprite();
    let registry = ComponentRegistry::with_defaults();

    assert!(registry.remove_one(&mut world, entity, "Sprite"));

    assert_eq!(world.get::<Sprite>(entity), None);
    assert!(
        world.get::<Transform>(entity).is_some(),
        "and leaves the others alone"
    );
}

#[test]
fn remove_one_is_false_for_an_unregistered_name() {
    let (mut world, entity) = world_with_one_sprite();
    let registry = ComponentRegistry::with_defaults();
    assert!(!registry.remove_one(&mut world, entity, "Physics"));
}

#[test]
fn removing_a_component_the_entity_lacks_is_not_an_error() {
    // The postcondition is "the entity does not have one", and it already
    // held. A caller making an entity match a record hits this for every
    // registered type the entity never had.
    let mut world = World::new();
    let bare = world.spawn();
    let registry = ComponentRegistry::with_defaults();
    assert!(registry.remove_one(&mut world, bare, "Sprite"));
}

#[test]
fn the_component_error_names_the_registered_name_not_the_rust_path() {
    // Regression test: this field used to hold `std::any::type_name::<T>()`
    // (e.g. `voltra_scene::sprite::Sprite`), which is not the name that
    // appears in a scene file, and does not let a user find the thing the
    // error is about. It must be the name `register` was called with.
    let mut world = World::new();
    let entity = world.spawn();
    let registry = ComponentRegistry::with_defaults();

    let nonsense = RawValue::from_ron("\"not a sprite\"").expect("valid RON");

    let result = registry
        .load_one(&mut world, entity, "Sprite", nonsense)
        .expect("Sprite is registered");
    match result {
        Err(SceneError::Component { name, .. }) => assert_eq!(name, "Sprite"),
        other => panic!("expected a Component error, got {other:?}"),
    }
}
