//! Which component types a scene file can carry, and how each converts.
//!
//! The registry is what lets serialization stay out of `voltra-ecs`. That crate
//! stores components type-erased behind a `TypeId`, with no way to enumerate the
//! types or reach one without knowing it at compile time — so the list of types
//! has to come from somewhere else. This is that somewhere: registering a type
//! captures functions that already know `T`, and saving then walks the registry
//! rather than the storages.

use ron::value::RawValue;
use serde::de::DeserializeOwned;
use serde::Serialize;
use voltra_ecs::{Entity, World};

use super::error::SceneError;
use crate::{
    Camera, Collider, CollisionLayers, Name, Parent, PhysicsMaterial, RigidBody, Sensor, Sprite,
    SpriteAnimation, Transform,
};

/// A registered type's save conversion: entity to stored value, or `None` when
/// the entity has no such component.
type SaveFn = fn(&World, Entity) -> Option<Result<Box<RawValue>, SceneError>>;

/// A registered type's load conversion: stored value to entity.
///
/// Returns the bare `ron` error rather than a [`SceneError`]: this function
/// only knows `T`, not the registered name the [`Entry`] was stored under, and
/// `SceneError::Component` needs that name to point at anything a user can act
/// on. `load_one` has the `Entry` in hand, so it wraps this into a `SceneError`
/// with the real name instead of `T`'s Rust path.
type LoadFn = fn(&mut World, Entity, &RawValue) -> Result<(), ron::error::SpannedError>;

/// A registered type's removal.
///
/// Needed because a record describes an entity *exactly*: putting one back has
/// to take off the components it does not carry, not only put on the ones it
/// does. Without this, undoing the addition of a component leaves it behind and
/// reports success.
type RemoveFn = fn(&mut World, Entity);

/// One component type's name and the three conversions that go with it.
///
/// `save`/`load`/`remove` are read by `save_one`/`load_one`/`remove_one` below.
struct Entry {
    name: &'static str,
    save: SaveFn,
    load: LoadFn,
    remove: RemoveFn,
}

/// The component types a scene file may contain.
///
/// Registration is explicit. A type that is not registered is not persisted,
/// which is a property worth choosing rather than inheriting.
pub struct ComponentRegistry {
    entries: Vec<Entry>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Every component type this crate defines.
    ///
    /// Registering here rather than making callers opt in is deliberate: there
    /// are several call sites, and forgetting one would not fail — it would
    /// silently drop that component from that save. A format that loses data
    /// when a caller forgets a line is not a format.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register::<Name>("Name");
        registry.register::<Transform>("Transform");
        // Stored as the parent's `SceneId`; the live `Entity` is resolved after
        // the whole file is in, by `hierarchy::resolve_parents`.
        registry.register::<Parent>("Parent");
        registry.register::<Sprite>("Sprite");
        registry.register::<SpriteAnimation>("SpriteAnimation");
        registry.register::<Camera>("Camera");
        registry.register::<RigidBody>("RigidBody");
        registry.register::<Collider>("Collider");
        registry.register::<PhysicsMaterial>("PhysicsMaterial");
        registry.register::<CollisionLayers>("CollisionLayers");
        registry.register::<Sensor>("Sensor");
        registry
    }

    /// Adds a component type under `name`, which is what appears in the file.
    ///
    /// The name is chosen rather than derived from the Rust path, so renaming a
    /// type in code does not silently invalidate every scene on disk.
    pub fn register<T>(&mut self, name: &'static str)
    where
        T: Serialize + DeserializeOwned + 'static,
    {
        self.entries.push(Entry {
            name,
            save: |world, entity| {
                let component = world.get::<T>(entity)?;
                Some(RawValue::from_rust(component).map_err(SceneError::Serialize))
            },
            load: |world, entity, value| {
                let component: T = value.into_rust()?;
                world.insert(entity, component);
                Ok(())
            },
            remove: |world, entity| {
                world.remove::<T>(entity);
            },
        });
    }

    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.iter().map(|e| e.name)
    }

    /// The entity's value for `name`, or `None` when the name is unregistered
    /// **or** the entity simply has no such component.
    pub(crate) fn save_one(
        &self,
        world: &World,
        entity: Entity,
        name: &str,
    ) -> Option<Result<Box<RawValue>, SceneError>> {
        let entry = self.entries.iter().find(|e| e.name == name)?;
        (entry.save)(world, entity)
    }

    /// Inserts `value` as `name`, or `None` when the name is unregistered.
    ///
    /// The outer `Option` is the caller's signal to preserve the value untouched
    /// instead of failing.
    pub(crate) fn load_one(
        &self,
        world: &mut World,
        entity: Entity,
        name: &str,
        value: &RawValue,
    ) -> Option<Result<(), SceneError>> {
        let entry = self.entries.iter().find(|e| e.name == name)?;
        Some(
            (entry.load)(world, entity, value).map_err(|source| SceneError::Component {
                name: entry.name.to_owned(),
                source: source.into(),
            }),
        )
    }

    /// Takes `name` off `entity`, returning whether the name was registered.
    ///
    /// Removing a component the entity does not have is not an error: the
    /// postcondition is "the entity does not have one", and it already held.
    pub(crate) fn remove_one(&self, world: &mut World, entity: Entity, name: &str) -> bool {
        let Some(entry) = self.entries.iter().find(|e| e.name == name) else {
            return false;
        };
        (entry.remove)(world, entity);
        true
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
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
}
