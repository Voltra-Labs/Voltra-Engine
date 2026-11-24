//! What a caller says about a type as it registers it, and what the
//! registry says back about each.

use ron::value::RawValue;
use serde::de::DeserializeOwned;
use serde::Serialize;

use super::entry::Entry;
use crate::format::error::SceneError;
use crate::format::refs::SceneRefs;

/// What the registry says about one component type, for a tool listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentInfo {
    /// The name a scene file stores it under.
    pub name: &'static str,
    /// Whether it names other entities by id.
    pub has_refs: bool,
    /// Whether [`ComponentRegistry::template`] answers for it.
    pub has_template: bool,
}

/// A type just registered: what else the registry should know of it.
///
/// Returned by [`ComponentRegistry::register`]; a caller that has nothing
/// to add ignores it.
pub struct Registration<'r, T> {
    entry: &'r mut Entry,
    _type: std::marker::PhantomData<fn() -> T>,
}

impl<'r, T> Registration<'r, T> {
    /// The registration of the type `entry` was just made for.
    pub(super) fn new(entry: &'r mut Entry) -> Self {
        Self {
            entry,
            _type: std::marker::PhantomData,
        }
    }

    /// Makes the type replicated, through `fns`.
    pub(super) fn set_replicate(&mut self, fns: super::replicate::ReplicateFns) {
        self.entry.replicate = Some(fns);
    }
}

impl<T> Registration<'_, T>
where
    T: Serialize + DeserializeOwned + 'static,
{
    /// Its `Default` is a sensible starting value: [`template`] answers it.
    ///
    /// [`template`]: ComponentRegistry::template
    pub fn with_template(self) -> Self
    where
        T: Default,
    {
        self.entry.template =
            Some(|| RawValue::from_rust(&T::default()).map_err(SceneError::Serialize));
        self
    }

    /// It names other entities by id: every copy and prefab compare
    /// rewrites them ([`ComponentRegistry::remap_refs`],
    /// [`ComponentRegistry::rewrite_refs`]).
    pub fn with_refs(self) -> Self
    where
        T: SceneRefs,
    {
        self.entry.refs = Some((
            |world, entity, map| {
                if let Some(component) = world.get_mut::<T>(entity) {
                    component.remap(map);
                }
            },
            |value, map| {
                let mut component: T = value.into_rust().map_err(SceneError::Parse)?;
                component.remap(map);
                RawValue::from_rust(&component).map_err(SceneError::Serialize)
            },
        ));
        self
    }
}
