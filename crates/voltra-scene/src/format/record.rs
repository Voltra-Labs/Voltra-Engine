//! One entity as a record, and a record back onto one entity.
//!
//! The same conversion [`super::save`] and [`super::load`] do, at the
//! granularity of a single entity rather than a whole file. Undo needs exactly
//! this: the state of the few entities an action touched, and a way to put it
//! back without disturbing the rest of the scene.

use std::collections::BTreeMap;

use ron::value::RawValue;
use voltra_ecs::{Entity, World};

use super::error::SceneError;
use super::registry::ComponentRegistry;
use super::save::EntityRecord;
use crate::scene_id::{SceneId, UnknownComponents};

/// The entity carrying `id`, if the world holds one.
///
/// Linear in the number of entities with a `SceneId`. An index would be faster
/// and is not yet worth the invalidation it would need on every spawn and
/// despawn: the callers are one selection per frame and one action per click.
pub fn entity_with_id(world: &World, id: SceneId) -> Option<Entity> {
    world
        .query::<SceneId>()
        .find(|(_, other)| **other == id)
        .map(|(entity, _)| entity)
}

/// Every component `entity` carries, registered or not.
///
/// `None` when the entity has no [`SceneId`]: a record is addressed by scene
/// identity, and an entity without one is transient by definition — the same
/// rule [`super::save::to_scene_file`] uses to decide what belongs in a file.
pub fn record_entity(
    world: &World,
    registry: &ComponentRegistry,
    entity: Entity,
) -> Option<Result<EntityRecord, SceneError>> {
    let id = *world.get::<SceneId>(entity)?;

    let mut components = BTreeMap::new();
    for name in registry.names() {
        match registry.save_one(world, entity, name) {
            Some(Ok(value)) => {
                components.insert(name.to_owned(), value);
            }
            Some(Err(error)) => return Some(Err(error)),
            None => {}
        }
    }

    // Merged after the known ones so a component that has since become
    // registered wins over the stale copy kept from an older load.
    if let Some(unknown) = world.get::<UnknownComponents>(entity) {
        for (name, value) in &unknown.0 {
            components
                .entry(name.clone())
                .or_insert_with(|| value.clone());
        }
    }

    Some(Ok(EntityRecord { id, components }))
}

/// [`record_entity`], addressed by scene identity.
pub fn record_scene_id(
    world: &World,
    registry: &ComponentRegistry,
    id: SceneId,
) -> Option<Result<EntityRecord, SceneError>> {
    record_entity(world, registry, entity_with_id(world, id)?)
}

/// Reads `components` onto `entity`, returning the ones no type claimed.
///
/// Shared with the loader, which does the same reading onto an entity it has
/// just spawned. Kept as its own function rather than folded into
/// [`apply_record`] because loading must always spawn and applying must not:
/// `Scene ▸ Open` loads the new scene before despawning the old one, so an id
/// present in both would land on the old entity and vanish with it.
pub(crate) fn insert_components(
    world: &mut World,
    registry: &ComponentRegistry,
    entity: Entity,
    components: &BTreeMap<String, Box<RawValue>>,
) -> Result<UnknownComponents, SceneError> {
    let mut unknown = UnknownComponents::default();

    for (name, value) in components {
        match registry.load_one(world, entity, name, value) {
            // Registered: a failure here is ours. The name says this build
            // understands the component, so data it cannot read is broken
            // rather than foreign.
            Some(Ok(())) => {}
            Some(Err(error)) => return Err(error),
            // Not registered: keep it verbatim so saving writes it back.
            // Dropping it is how a build silently deletes work done by a build
            // that knew more.
            None => {
                log::warn!("scene contains unknown component `{name}`; keeping it unread");
                unknown.0.insert(name.clone(), value.clone());
            }
        }
    }

    Ok(unknown)
}

/// Makes the world hold an entity **equal** to `record`, spawning one if the id
/// is not there.
///
/// Equal, not "at least": a registered component the record does not carry is
/// removed, and [`UnknownComponents`] goes with it when the record has none.
/// Anything less makes undoing the addition of a component a partial undo that
/// reports success.
pub fn apply_record(
    world: &mut World,
    registry: &ComponentRegistry,
    record: &EntityRecord,
) -> Result<Entity, SceneError> {
    let entity = match entity_with_id(world, record.id) {
        Some(entity) => entity,
        None => {
            let entity = world.spawn();
            world.insert(entity, record.id);
            entity
        }
    };

    let unknown = insert_components(world, registry, entity, &record.components)?;

    for name in registry.names() {
        if !record.components.contains_key(name) {
            registry.remove_one(world, entity, name);
        }
    }

    if unknown.0.is_empty() {
        world.remove::<UnknownComponents>(entity);
    } else {
        world.insert(entity, unknown);
    }

    // A record carries a parent as a `SceneId`, and the `Entity` it resolved to
    // when the record was taken may not exist any more — an undo that spawns an
    // entity back gives it a new handle. Resolving the whole world rather than
    // this one link is deliberate: the entity coming back may itself be some
    // other entity's parent, and that link is just as stale.
    crate::hierarchy::resolve_parents(world);

    Ok(entity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Sprite, Transform};
    use voltra_render::glam::Vec2;

    fn world_with_sprite() -> (World, Entity, SceneId) {
        let mut world = World::new();
        let entity = world.spawn();
        let id = SceneId::new();
        world.insert(entity, id);
        world.insert(entity, Transform::from_translation(Vec2::new(1.0, 2.0)));
        world.insert(entity, Sprite::default().with_sort_order(3));
        (world, entity, id)
    }

    #[test]
    fn an_entity_without_a_scene_id_has_no_record() {
        let mut world = World::new();
        let bare = world.spawn();
        world.insert(bare, Transform::default());
        assert!(record_entity(&world, &ComponentRegistry::with_defaults(), bare).is_none());
    }

    #[test]
    fn a_record_carries_the_entitys_components() {
        let (world, entity, id) = world_with_sprite();
        let record = record_entity(&world, &ComponentRegistry::with_defaults(), entity)
            .expect("the entity has a SceneId")
            .expect("its components serialize");

        assert_eq!(record.id, id);
        let names: Vec<_> = record.components.keys().cloned().collect();
        assert_eq!(names, vec!["Sprite".to_owned(), "Transform".to_owned()]);
    }

    #[test]
    fn applying_a_record_to_an_empty_world_spawns_the_entity() {
        let (source, entity, id) = world_with_sprite();
        let registry = ComponentRegistry::with_defaults();
        let record = record_entity(&source, &registry, entity)
            .expect("has an id")
            .expect("serializes");

        let mut target = World::new();
        let spawned = apply_record(&mut target, &registry, &record).expect("a fresh apply");

        assert_eq!(target.get::<SceneId>(spawned), Some(&id));
        assert_eq!(
            target.get::<Transform>(spawned),
            source.get::<Transform>(entity)
        );
        assert_eq!(target.get::<Sprite>(spawned), source.get::<Sprite>(entity));
    }

    #[test]
    fn applying_a_record_reuses_the_entity_that_already_carries_the_id() {
        let (mut world, entity, _) = world_with_sprite();
        let registry = ComponentRegistry::with_defaults();
        let record = record_entity(&world, &registry, entity)
            .expect("has an id")
            .expect("serializes");

        world
            .get_mut::<Transform>(entity)
            .expect("it has one")
            .translation = Vec2::new(9.0, 9.0);
        let same = apply_record(&mut world, &registry, &record).expect("apply");

        assert_eq!(same, entity, "the id must not gain a second entity");
        assert_eq!(world.entity_count(), 1);
        assert_eq!(
            world.get::<Transform>(entity).map(|t| t.translation),
            Some(Vec2::new(1.0, 2.0)),
            "the record's value must win"
        );
    }

    #[test]
    fn applying_a_record_removes_a_component_it_does_not_carry() {
        // The undo of an add-component. Without the removal step the component
        // survives and the undo is a partial one that reports success.
        let (mut world, entity, _) = world_with_sprite();
        let registry = ComponentRegistry::with_defaults();
        let mut record = record_entity(&world, &registry, entity)
            .expect("has an id")
            .expect("serializes");
        record.components.remove("Sprite");

        apply_record(&mut world, &registry, &record).expect("apply");

        assert_eq!(world.get::<Sprite>(entity), None);
        assert!(
            world.get::<Transform>(entity).is_some(),
            "and only that one"
        );
    }

    #[test]
    fn unknown_components_survive_a_record_round_trip() {
        let (mut world, entity, _) = world_with_sprite();
        let registry = ComponentRegistry::with_defaults();
        let physics: Box<RawValue> = RawValue::from_ron("(mass: 5.0)")
            .expect("valid RON")
            .to_owned();
        let mut unknown = UnknownComponents::default();
        unknown.0.insert("Physics".to_owned(), physics.clone());
        world.insert(entity, unknown);

        let record = record_entity(&world, &registry, entity)
            .expect("has an id")
            .expect("serializes");
        let mut target = World::new();
        let spawned = apply_record(&mut target, &registry, &record).expect("apply");

        assert_eq!(
            target
                .get::<UnknownComponents>(spawned)
                .map(|u| u.0.get("Physics").cloned()),
            Some(Some(physics)),
        );
    }

    #[test]
    fn applying_a_record_without_unknowns_drops_the_ones_the_entity_had() {
        let (mut world, entity, _) = world_with_sprite();
        let registry = ComponentRegistry::with_defaults();
        let record = record_entity(&world, &registry, entity)
            .expect("has an id")
            .expect("serializes");

        let mut unknown = UnknownComponents::default();
        unknown.0.insert(
            "Physics".to_owned(),
            RawValue::from_ron("(mass: 5.0)")
                .expect("valid RON")
                .to_owned(),
        );
        world.insert(entity, unknown);

        apply_record(&mut world, &registry, &record).expect("apply");

        assert!(
            world.get::<UnknownComponents>(entity).is_none(),
            "a record with no unknowns describes an entity with none"
        );
    }

    #[test]
    fn record_scene_id_finds_the_entity() {
        let (world, _, id) = world_with_sprite();
        let record = record_scene_id(&world, &ComponentRegistry::with_defaults(), id)
            .expect("the id is in the world")
            .expect("serializes");
        assert_eq!(record.id, id);
    }

    #[test]
    fn record_scene_id_is_none_for_an_absent_id() {
        let (world, _, _) = world_with_sprite();
        assert!(
            record_scene_id(&world, &ComponentRegistry::with_defaults(), SceneId::new()).is_none()
        );
    }

    #[test]
    fn entity_with_id_is_none_for_an_id_nothing_carries() {
        let (world, _, _) = world_with_sprite();
        assert!(entity_with_id(&world, SceneId::new()).is_none());
    }
}
