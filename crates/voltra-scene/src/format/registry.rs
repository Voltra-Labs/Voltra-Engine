//! Which component types a scene file can carry, and how each converts.
//!
//! The registry is what lets serialization stay out of `voltra-ecs`. That crate
//! stores components type-erased behind a `TypeId`, with no way to enumerate the
//! types or reach one without knowing it at compile time — so the list of types
//! has to come from somewhere else. This is that somewhere: registering a type
//! captures functions that already know `T`, and saving then walks the registry
//! rather than the storages.

use serde::de::DeserializeOwned;
use serde::Serialize;
use voltra_ecs::{Entity, World};

use super::error::SceneError;
use crate::{Sprite, Transform};

/// One component type's name and the two conversions that go with it.
///
/// `save`/`load` are read by `save_one`/`load_one` below. Nothing outside
/// tests calls those yet — scene save/load, which wires them up, is a later
/// task — so `dead_code` sees no live root and flags the fields transitively.
/// Allowed rather than worked around, since the fix is a task away, not a bug.
#[allow(dead_code)]
struct Entry {
    name: &'static str,
    save: fn(&World, Entity) -> Option<Result<ron::Value, SceneError>>,
    load: fn(&mut World, Entity, &ron::Value) -> Result<(), SceneError>,
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
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register::<Transform>("Transform");
        registry.register::<Sprite>("Sprite");
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
                Some(to_value(component))
            },
            load: |world, entity, value| {
                let component: T =
                    value
                        .clone()
                        .into_rust()
                        .map_err(|source| SceneError::Component {
                            name: std::any::type_name::<T>().to_owned(),
                            source,
                        })?;
                world.insert(entity, component);
                Ok(())
            },
        });
    }

    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.iter().map(|e| e.name)
    }

    /// The entity's value for `name`, or `None` when the name is unregistered
    /// **or** the entity simply has no such component.
    ///
    /// Only tests call this so far; scene save wires it up in a later task.
    #[allow(dead_code)]
    pub(crate) fn save_one(
        &self,
        world: &World,
        entity: Entity,
        name: &str,
    ) -> Option<Result<ron::Value, SceneError>> {
        let entry = self.entries.iter().find(|e| e.name == name)?;
        (entry.save)(world, entity)
    }

    /// Inserts `value` as `name`, or `None` when the name is unregistered.
    ///
    /// The outer `Option` is the caller's signal to preserve the value untouched
    /// instead of failing.
    ///
    /// Only tests call this so far; scene load wires it up in a later task.
    #[allow(dead_code)]
    pub(crate) fn load_one(
        &self,
        world: &mut World,
        entity: Entity,
        name: &str,
        value: &ron::Value,
    ) -> Option<Result<(), SceneError>> {
        let entry = self.entries.iter().find(|e| e.name == name)?;
        Some((entry.load)(world, entity, value))
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// A component as a `ron::Value`, via RON text.
///
/// `Value` is the one representation both known and unknown components share,
/// which is what lets a single code path write both.
fn to_value<T: Serialize>(component: &T) -> Result<ron::Value, SceneError> {
    let text = ron::to_string(component).map_err(SceneError::Serialize)?;
    ron::from_str(&text).map_err(SceneError::Parse)
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
        let value = ron::Value::Unit;
        assert!(registry
            .load_one(&mut world, entity, "Physics", &value)
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
        let nonsense: ron::Value = ron::from_str("\"not a sprite\"").expect("valid RON");

        let result = registry
            .load_one(&mut world, entity, "Sprite", &nonsense)
            .expect("Sprite is registered");
        assert!(
            matches!(result, Err(SceneError::Component { .. })),
            "expected a Component error, got {result:?}"
        );
    }
}
