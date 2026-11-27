//! Registering, saving, loading and removing components by name.

use super::*;
use crate::{Sprite2D, Transform2D};
use voltra_render::glam::Vec2;

fn world_with_one_sprite() -> (World, Entity) {
    let mut world = World::new();
    let e = world.spawn();
    world.insert(e, Transform2D::from_translation(Vec2::new(1.0, 2.0)));
    world.insert(e, Sprite2D::default().with_sort_order(3));
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

    assert_eq!(target.get::<Sprite2D>(copy), world.get::<Sprite2D>(entity));
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

    assert_eq!(world.get::<Sprite2D>(entity), None);
    assert!(
        world.get::<Transform2D>(entity).is_some(),
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
fn has_asks_the_world_by_name() {
    let (world, entity) = world_with_one_sprite();
    let registry = ComponentRegistry::with_defaults();
    assert_eq!(registry.has(&world, entity, "Sprite"), Some(true));
    assert_eq!(registry.has(&world, entity, "Collider"), Some(false));
    assert_eq!(
        registry.has(&world, entity, "Physics"),
        None,
        "unregistered"
    );
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
    // (e.g. `voltra_scene::sprite::Sprite2D`), which is not the name that
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

#[test]
fn every_built_in_but_structure_has_a_template_that_loads_back() {
    let registry = ComponentRegistry::with_defaults();
    for info in registry.describe() {
        let structure = ["Parent", "PrefabInstance", "PrefabPart"].contains(&info.name);
        assert_eq!(info.has_template, !structure, "{}", info.name);
        let Some(template) = registry.template(info.name) else {
            continue;
        };
        let template = template.expect("a default writes");
        let mut world = World::new();
        let entity = world.spawn();
        registry
            .load_one(&mut world, entity, info.name, &template)
            .expect("registered")
            .unwrap_or_else(|e| panic!("{} reads its own template: {e}", info.name));
        let back = registry
            .save_one(&world, entity, info.name)
            .expect("registered")
            .expect("saves");
        assert_eq!(
            back.get_ron(),
            template.get_ron(),
            "{} round trips",
            info.name
        );
    }
}

#[test]
fn describe_says_which_name_entities_and_template_asks_only_for_known_names() {
    let registry = ComponentRegistry::with_defaults();
    let info = |name: &str| {
        registry
            .describe()
            .into_iter()
            .find(|i| i.name == name)
            .unwrap_or_else(|| panic!("{name} is registered"))
    };
    assert!(info("Joint2D").has_refs && info("CameraFollow2D").has_refs);
    assert!(!info("Sprite").has_refs);
    assert!(registry.template("Physics").is_none());
    assert!(registry.template("Parent").is_none());
}

#[test]
fn a_game_type_without_a_default_still_registers() {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Coin(u32);
    let mut registry = ComponentRegistry::new();
    registry.register::<Coin>("Coin");
    assert!(registry.names().any(|n| n == "Coin"));
    assert!(registry.template("Coin").is_none());
}

#[test]
fn every_built_in_round_trips_through_a_tree() {
    // The tree replays the type's own calls, so what it loads back is the
    // value, for every template the registry has.
    let registry = ComponentRegistry::with_defaults();
    for info in registry.describe().iter().filter(|i| i.has_template) {
        let mut world = World::new();
        let entity = world.spawn();
        let template = registry
            .template(info.name)
            .expect("listed")
            .expect("writes");
        assert!(matches!(
            registry.load_one(&mut world, entity, info.name, &template),
            Some(Ok(()))
        ));
        let tree = registry
            .save_tree(&world, entity, info.name)
            .expect("held")
            .expect("records");
        let other = world.spawn();
        assert!(
            matches!(
                registry.load_tree(&mut world, other, info.name, &tree),
                Some(Ok(()))
            ),
            "{} loads back from its tree",
            info.name
        );
        let first = registry.save_one(&world, entity, info.name).expect("h");
        let second = registry.save_one(&world, other, info.name).expect("h");
        assert_eq!(
            first.expect("w").get_ron(),
            second.expect("w").get_ron(),
            "{}",
            info.name
        );
    }
}

#[test]
fn a_tree_is_none_for_an_unregistered_name_or_a_missing_component() {
    let (mut world, entity) = world_with_one_sprite();
    let registry = ComponentRegistry::with_defaults();
    assert!(registry.save_tree(&world, entity, "Physics").is_none());
    assert!(registry.save_tree(&world, entity, "Camera").is_none());
    let tree = registry
        .save_tree(&world, entity, "Sprite")
        .expect("held")
        .expect("records");
    assert!(registry
        .load_tree(&mut world, entity, "Physics", &tree)
        .is_none());
    // A sprite's tree is no transform.
    assert!(matches!(
        registry.load_tree(&mut world, entity, "Transform", &tree),
        Some(Err(_))
    ));
}
