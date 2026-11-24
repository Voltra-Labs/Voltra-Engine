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

mod entry;
mod registration;

pub use registration::{ComponentInfo, Registration};

use entry::Entry;

use super::error::SceneError;
use super::refs::SceneRefs;
use crate::scene_id::SceneId;
use crate::{
    AudioListener, AudioSource, Camera, Collider, CollisionLayers, GlobalLight2D, Name, Parallax,
    Parent, ParticleEmitter, Persistent, PhysicsMaterial, PointLight2D, PrefabInstance, RigidBody,
    Sensor, ShadowCaster2D, Sprite, SpriteAnimation, Text, Tilemap, Transform, UiButton, UiRect,
    UiScroll, UiSlice, UiSlider, UiSliderPart, UiStack, UiTextField, UiToggle,
};

/// The component types a scene file may contain.
///
/// Registration is explicit. A type that is not registered is not persisted,
/// which is a property worth choosing rather than inheriting.
pub struct ComponentRegistry {
    entries: Vec<Entry>,
}

impl ComponentRegistry {
    /// An empty registry, with no component registered.
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
        registry.register::<Name>("Name").with_template();
        registry.register::<Transform>("Transform").with_template();
        // Stored as the parent's `SceneId`; the live `Entity` is resolved after
        // the whole file is in, by `hierarchy::resolve_parents`.
        registry.register::<Parent>("Parent");
        registry.register::<Sprite>("Sprite").with_template();
        registry
            .register::<SpriteAnimation>("SpriteAnimation")
            .with_template();
        registry
            .register::<crate::Animator>(crate::Animator::NAME)
            .with_template();
        registry
            .register::<crate::AnimationPlayer>(crate::AnimationPlayer::NAME)
            .with_template();
        registry.register::<Camera>("Camera").with_template();
        registry
            .register_with_refs::<crate::CameraFollow2D>(crate::CameraFollow2D::NAME)
            .with_template();
        registry
            .register::<crate::CameraShake>(crate::CameraShake::NAME)
            .with_template();
        registry
            .register::<AudioSource>("AudioSource")
            .with_template();
        registry
            .register::<AudioListener>("AudioListener")
            .with_template();
        registry.register::<RigidBody>("RigidBody").with_template();
        registry.register::<Collider>("Collider").with_template();
        registry
            .register::<PhysicsMaterial>("PhysicsMaterial")
            .with_template();
        registry
            .register::<CollisionLayers>("CollisionLayers")
            .with_template();
        registry.register::<Sensor>("Sensor").with_template();
        registry
            .register::<crate::OneWay>(crate::OneWay::NAME)
            .with_template();
        registry
            .register::<crate::CharacterBody2D>(crate::CharacterBody2D::NAME)
            .with_template();
        registry.register::<PrefabInstance>(PrefabInstance::NAME);
        registry.register::<crate::prefab::PrefabPart>(crate::prefab::PrefabPart::NAME);
        registry.register::<Tilemap>("Tilemap").with_template();
        registry.register::<Text>("Text").with_template();
        registry.register::<UiRect>("UiRect").with_template();
        registry.register::<UiButton>("UiButton").with_template();
        registry.register::<UiSlice>("UiSlice").with_template();
        registry.register::<UiStack>("UiStack").with_template();
        registry
            .register::<crate::CameraTarget>("CameraTarget")
            .with_template();
        registry.register::<UiScroll>("UiScroll").with_template();
        registry.register::<UiToggle>("UiToggle").with_template();
        registry.register::<UiSlider>("UiSlider").with_template();
        registry
            .register::<UiSliderPart>("UiSliderPart")
            .with_template();
        registry
            .register::<UiTextField>("UiTextField")
            .with_template();
        registry
            .register::<Persistent>("Persistent")
            .with_template();
        registry.register::<Parallax>("Parallax").with_template();
        registry
            .register::<ParticleEmitter>("ParticleEmitter")
            .with_template();
        registry
            .register::<PointLight2D>("PointLight2D")
            .with_template();
        registry
            .register::<GlobalLight2D>("GlobalLight2D")
            .with_template();
        registry
            .register_with_refs::<crate::Joint2D>(crate::Joint2D::NAME)
            .with_template();
        registry
            .register::<ShadowCaster2D>("ShadowCaster2D")
            .with_template();
        registry.register::<crate::Line2D>("Line2D").with_template();
        registry
            .register::<crate::Polygon2D>("Polygon2D")
            .with_template();
        registry
            .register::<crate::Trail2D>("Trail2D")
            .with_template();
        registry
            .register::<crate::Skeleton2D>("Skeleton2D")
            .with_template();
        registry.register::<crate::Bone2D>("Bone2D").with_template();
        registry
            .register::<crate::TwoBoneIk>("TwoBoneIk")
            .with_template();
        registry
            .register::<crate::SkinnedSprite>("SkinnedSprite")
            .with_template();
        registry
    }

    /// Adds a component type under `name`, which is what appears in the file.
    ///
    /// The name is chosen rather than derived from the Rust path, so renaming a
    /// type in code does not silently invalidate every scene on disk.
    pub fn register<T>(&mut self, name: &'static str) -> Registration<'_, T>
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
            normalize: |value| {
                let component: T = value.into_rust().map_err(SceneError::Parse)?;
                RawValue::from_rust(&component).map_err(SceneError::Serialize)
            },
            refs: None,
            template: None,
        });
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

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests;
