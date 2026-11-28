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

mod defaults;
mod entry;
mod registration;
mod replicate;
mod tree;

pub use registration::{ComponentInfo, Registration};
pub use replicate::{ReplicateError, ReplicatedComponent};

use entry::Entry;

use super::error::SceneError;
use super::refs::SceneRefs;
use super::serial_tree::SerialTree;
use crate::scene_id::SceneId;

/// The component types a scene file may contain.
///
/// Registration is explicit. A type that is not registered is not persisted,
/// which is a property worth choosing rather than inheriting.
#[derive(Clone)]
pub struct ComponentRegistry {
    entries: Vec<Entry>,
    /// Names registered a second time, refused: the first type keeps each.
    refused: Vec<&'static str>,
    /// Where a refused registration's builder calls land, kept out of
    /// `entries` so nothing reads them.
    discarded: Option<Box<Entry>>,
    /// Which registration this is: new for every registry and every change
    /// to one, kept by a clone — so a holder of a copy can tell it is stale.
    stamp: u64,
}

/// The stamps handed out so far.
static STAMPS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn next_stamp() -> u64 {
    STAMPS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl ComponentRegistry {
    /// An empty registry, with no component registered.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            refused: Vec::new(),
            discarded: None,
            stamp: next_stamp(),
        }
    }

    /// The same for a registry and its clones until either changes: what
    /// a store holding a copy (the prefab store, which reads stored
    /// instances through one) checks to know its copy is out of date.
    pub fn stamp(&self) -> u64 {
        self.stamp
    }

    /// Adds a component type under `name`, which is what appears in the file.
    ///
    /// The name is chosen rather than derived from the Rust path, so renaming a
    /// type in code does not silently invalidate every scene on disk.
    ///
    /// A name already registered is **refused**: the first type keeps it,
    /// this one is not registered, the refusal is logged as an error naming
    /// both types, and [`refused`](Self::refused) lists it. Two types under
    /// one name would make every file holding it read as whichever came
    /// first — a game's `Sprite2D` shadowing the engine's, or the reverse.
    pub fn register<T>(&mut self, name: &'static str) -> Registration<'_, T>
    where
        T: Serialize + DeserializeOwned + 'static,
    {
        self.stamp = next_stamp();
        let entry = Entry {
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
            has: |world, entity| world.get::<T>(entity).is_some(),
            normalize: |value| {
                let component: T = value.into_rust().map_err(SceneError::Parse)?;
                RawValue::from_rust(&component).map_err(SceneError::Serialize)
            },
            tree: |world, entity| world.get::<T>(entity).map(SerialTree::record),
            refs: None,
            template: None,
            type_id: std::any::TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
            replicate: None,
        };
        if let Some(first) = self.entries.iter().find(|e| e.name == name) {
            log::error!(
                "component name `{name}` is registered already (by {}); {} is not \
                 registered: give one of them another name",
                first.type_name,
                std::any::type_name::<T>()
            );
            self.refused.push(name);
            return Registration::new(self.discarded.insert(Box::new(entry)));
        }
        self.entries.push(entry);
        Registration::new(
            self.entries
                .last_mut()
                .expect("register just pushed the entry"),
        )
    }

    /// [`register`](Self::register) with [`Registration::with_refs`], for
    /// a type that names other entities by id.
    pub fn register_with_refs<T>(&mut self, name: &'static str) -> Registration<'_, T>
    where
        T: Serialize + DeserializeOwned + SceneRefs + 'static,
    {
        self.register::<T>(name).with_refs()
    }

    /// Rewrites, through `map`, every id the components on `entity` name.
    pub fn remap_refs(
        &self,
        world: &mut World,
        entity: Entity,
        map: &dyn Fn(SceneId) -> Option<SceneId>,
    ) {
        for (remap, _) in self.entries.iter().filter_map(|e| e.refs) {
            remap(world, entity, map);
        }
    }

    /// `value`, stored as `name`, with the ids it names taken through `map`
    /// — `None` for a type that names none, or one not registered.
    pub(crate) fn rewrite_refs(
        &self,
        name: &str,
        value: &RawValue,
        map: &dyn Fn(SceneId) -> Option<SceneId>,
    ) -> Result<Option<Box<RawValue>>, SceneError> {
        let Some(entry) = self.entries.iter().find(|e| e.name == name) else {
            return Ok(None);
        };
        let Some((_, rewrite)) = entry.refs else {
            return Ok(None);
        };
        rewrite(value, map).map(Some).map_err(|error| match error {
            SceneError::Parse(source) => SceneError::Component {
                name: entry.name.to_owned(),
                source: source.into(),
            },
            other => other,
        })
    }

    /// The names a [`register`](Self::register) call was refused, because
    /// another type had it first; empty when every name is one type's.
    pub fn refused(&self) -> &[&'static str] {
        &self.refused
    }

    /// The registered component names, in registration order.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.iter().map(|e| e.name)
    }

    /// Every registered type, in registration order, and what is known of
    /// it.
    pub fn describe(&self) -> Vec<ComponentInfo> {
        self.entries
            .iter()
            .map(|e| ComponentInfo {
                name: e.name,
                has_refs: e.refs.is_some(),
                has_template: e.template.is_some(),
            })
            .collect()
    }

    /// `name`'s default value, as a scene file writes it: a starting point
    /// to edit and hand back to [`load_one`](Self::load_one). `None` for an
    /// unregistered name, or a type registered without a template.
    pub fn template(&self, name: &str) -> Option<Result<Box<RawValue>, SceneError>> {
        let entry = self.entries.iter().find(|e| e.name == name)?;
        entry.template.map(|make| make())
    }

    /// The entity's value for `name`, or `None` when the name is unregistered
    /// **or** the entity simply has no such component.
    pub fn save_one(
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
    pub fn load_one(
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

    /// `value` as `name`'s type writes it — what [`save_one`](Self::save_one)
    /// would answer for an entity holding it — so two values are equal
    /// exactly when they are: one read from a pretty-printed file and one
    /// taken from a world compare as text. `None` for an unregistered name.
    pub fn normalize(
        &self,
        name: &str,
        value: &RawValue,
    ) -> Option<Result<Box<RawValue>, SceneError>> {
        let entry = self.entries.iter().find(|e| e.name == name)?;
        Some((entry.normalize)(value).map_err(|error| match error {
            // Named, as `load_one` names a value it cannot read.
            SceneError::Parse(source) => SceneError::Component {
                name: entry.name.to_owned(),
                source: source.into(),
            },
            other => other,
        }))
    }

    /// Whether `entity` has the component registered as `name`, or `None`
    /// for an unregistered name. Asks the world, writing nothing out — what
    /// the editor's `t:` search runs over every entity.
    pub fn has(&self, world: &World, entity: Entity, name: &str) -> Option<bool> {
        let entry = self.entries.iter().find(|e| e.name == name)?;
        Some((entry.has)(world, entity))
    }

    /// Takes `name` off `entity`, returning whether the name was registered.
    ///
    /// Removing a component the entity does not have is not an error: the
    /// postcondition is "the entity does not have one", and it already held.
    pub fn remove_one(&self, world: &mut World, entity: Entity, name: &str) -> bool {
        let Some(entry) = self.entries.iter().find(|e| e.name == name) else {
            return false;
        };
        (entry.remove)(world, entity);
        true
    }
}

impl std::fmt::Debug for ComponentRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentRegistry")
            .field("types", &self.entries.len())
            .field("stamp", &self.stamp)
            .finish()
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests;
