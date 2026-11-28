//! Which registered types the server sends its clients, and the compact
//! encoding they travel in.
//!
//! A scene file stores a component as RON — readable, merged in git, and
//! about three times the size of the value. On the wire the same value is
//! `postcard`: varints and raw floats, no field names, so a `Transform2D` is
//! 20 bytes. Both conversions are captured at registration, as the RON ones
//! are, so replication walks the registry and never names a type.

use std::fmt;

use serde::de::DeserializeOwned;
use serde::Serialize;
use voltra_ecs::{Entity, World};

use super::registration::Registration;
use super::ComponentRegistry;
use crate::interpolate::Interpolate;

/// The value on `entity`, encoded; `None` when it has none.
type EncodeFn = fn(&World, Entity) -> Option<Result<Vec<u8>, postcard::Error>>;
/// An encoded value inserted on `entity`, replacing any there.
type ApplyFn = fn(&mut World, Entity, &[u8]) -> Result<(), postcard::Error>;
/// Whether any entity of the world has one.
type PresentFn = fn(&World) -> bool;
/// The value a fraction `t` of the way from one encoded value to another.
type LerpFn = fn(&[u8], &[u8], f32) -> Result<Vec<u8>, postcard::Error>;

/// What the registry keeps for a replicated type.
#[derive(Clone, Copy)]
pub(super) struct ReplicateFns {
    encode: EncodeFn,
    apply: ApplyFn,
    present: PresentFn,
    lerp: Option<LerpFn>,
}

impl ReplicateFns {
    fn of<T: Serialize + DeserializeOwned + 'static>() -> Self {
        Self {
            encode: |world, entity| world.get::<T>(entity).map(postcard::to_allocvec),
            present: |world| world.component_count::<T>() > 0,
            apply: |world, entity, bytes| {
                let value: T = decode(bytes)?;
                world.insert(entity, value);
                Ok(())
            },
            lerp: None,
        }
    }

    fn interpolated<T: Serialize + DeserializeOwned + Interpolate + 'static>() -> Self {
        Self {
            lerp: Some(|from, to, t| {
                let from: T = decode(from)?;
                let to: T = decode(to)?;
                postcard::to_allocvec(&from.interpolate(&to, t))
            }),
            ..Self::of::<T>()
        }
    }
}

/// One value, and nothing after it: bytes from the network are untrusted,
/// and trailing ones mean the sender and we disagree about the type.
fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, postcard::Error> {
    let (value, rest) = postcard::take_from_bytes(bytes)?;
    if rest.is_empty() {
        Ok(value)
    } else {
        Err(postcard::Error::DeserializeBadEncoding)
    }
}

/// A replicated type, as [`ComponentRegistry::replicated`] lists it: the
/// handle every replication call takes.
///
/// Cheap to copy and valid for the registry it came from; an index into
/// another registry is a wrong answer, not a crash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReplicatedComponent {
    entry: usize,
    /// The name a scene file stores it under — what the server and its
    /// clients agree on, rather than an index each counts on its own.
    pub name: &'static str,
    /// Whether remote copies are drawn between snapshots rather than snapped
    /// to each: registered with
    /// [`replicate_interpolated`](ComponentRegistry::replicate_interpolated).
    pub interpolated: bool,
}

/// Why a replication call could not be done.
#[derive(Debug)]
pub enum ReplicateError {
    /// `replicate::<T>()` for a `T` never registered: it has no name to
    /// travel under. Register it first.
    NotRegistered(&'static str),
    /// Bytes that are not a value of the type — a peer on another version,
    /// or junk.
    Codec(postcard::Error),
    /// A handle this registry did not issue.
    UnknownHandle,
}

impl fmt::Display for ReplicateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRegistered(name) => {
                write!(f, "{name} is not registered, so it cannot be replicated")
            }
            Self::Codec(error) => write!(f, "not a value of the component: {error}"),
            Self::UnknownHandle => write!(f, "a replicated component this registry did not list"),
        }
    }
}

impl std::error::Error for ReplicateError {}

impl From<postcard::Error> for ReplicateError {
    fn from(error: postcard::Error) -> Self {
        Self::Codec(error)
    }
}

impl<T> Registration<'_, T>
where
    T: Serialize + DeserializeOwned + 'static,
{
    /// The server sends it to its clients (see
    /// [`ComponentRegistry::replicate`]).
    pub fn replicated(mut self) -> Self {
        self.set_replicate(ReplicateFns::of::<T>());
        self
    }

    /// The server sends it, and remote copies are drawn between snapshots
    /// (see [`ComponentRegistry::replicate_interpolated`]).
    pub fn replicated_interpolated(mut self) -> Self
    where
        T: Interpolate,
    {
        self.set_replicate(ReplicateFns::interpolated::<T>());
        self
    }
}

impl ComponentRegistry {
    /// Marks the registered type `T` as replicated: on an entity carrying
    /// [`Replicated`](crate::Replicated), the server sends its value to
    /// every client when it changes. `Transform2D` and `Transform3D` are by
    /// default, both interpolated.
    ///
    /// Opt-in per type, as Unreal's `UPROPERTY(Replicated)` and Bevy
    /// replicon's `replicate::<C>()` are: most components (a sprite's
    /// texture, a collider's shape) never change in play and are made on
    /// each end from the same prefab.
    pub fn replicate<T>(&mut self) -> Result<(), ReplicateError>
    where
        T: Serialize + DeserializeOwned + 'static,
    {
        self.set_replicate::<T>(ReplicateFns::of::<T>())
    }

    /// [`replicate`](Self::replicate), and remote copies are drawn between
    /// the two snapshots around the client's render time rather than
    /// snapped to the newest.
    pub fn replicate_interpolated<T>(&mut self) -> Result<(), ReplicateError>
    where
        T: Serialize + DeserializeOwned + Interpolate + 'static,
    {
        self.set_replicate::<T>(ReplicateFns::interpolated::<T>())
    }

    fn set_replicate<T: 'static>(&mut self, fns: ReplicateFns) -> Result<(), ReplicateError> {
        let id = std::any::TypeId::of::<T>();
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.type_id == id)
            .ok_or(ReplicateError::NotRegistered(std::any::type_name::<T>()))?;
        entry.replicate = Some(fns);
        self.stamp = super::next_stamp();
        Ok(())
    }

    /// Every replicated type, in registration order.
    pub fn replicated(&self) -> Vec<ReplicatedComponent> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(entry, e)| {
                e.replicate.map(|fns| ReplicatedComponent {
                    entry,
                    name: e.name,
                    interpolated: fns.lerp.is_some(),
                })
            })
            .collect()
    }

    fn fns(&self, component: ReplicatedComponent) -> Result<ReplicateFns, ReplicateError> {
        self.entries
            .get(component.entry)
            .filter(|e| e.name == component.name)
            .and_then(|e| e.replicate)
            .ok_or(ReplicateError::UnknownHandle)
    }

    /// `entity`'s value of `component`, encoded for the wire; `None` when it
    /// has none.
    pub fn encode_replicated(
        &self,
        component: ReplicatedComponent,
        world: &World,
        entity: Entity,
    ) -> Option<Result<Vec<u8>, ReplicateError>> {
        match self.fns(component) {
            Ok(fns) => (fns.encode)(world, entity).map(|r| r.map_err(ReplicateError::from)),
            Err(error) => Some(Err(error)),
        }
    }

    /// Whether any entity of `world` has `component` — one storage's length,
    /// so a server reading the world once a snapshot can pass over a type
    /// no entity has (a 2D game's `Transform3D`) instead of asking every
    /// entity for it. `false` for a handle this registry did not issue.
    pub fn any_replicated(&self, component: ReplicatedComponent, world: &World) -> bool {
        self.fns(component).is_ok_and(|fns| (fns.present)(world))
    }

    /// Decodes `bytes` as `component` and puts it on `entity`, replacing
    /// any value there. Nothing changes when they will not decode.
    pub fn apply_replicated(
        &self,
        component: ReplicatedComponent,
        world: &mut World,
        entity: Entity,
        bytes: &[u8],
    ) -> Result<(), ReplicateError> {
        let fns = self.fns(component)?;
        (fns.apply)(world, entity, bytes).map_err(ReplicateError::from)
    }

    /// Takes `component` off `entity`, as the server says it no longer
    /// has one.
    pub fn remove_replicated(
        &self,
        component: ReplicatedComponent,
        world: &mut World,
        entity: Entity,
    ) {
        if let Some(entry) = self.entries.get(component.entry) {
            (entry.remove)(world, entity);
        }
    }

    /// The encoded value `t` of the way from `from` to `to`, for an
    /// interpolated type; `None` for one that is not.
    pub fn lerp_replicated(
        &self,
        component: ReplicatedComponent,
        from: &[u8],
        to: &[u8],
        t: f32,
    ) -> Option<Result<Vec<u8>, ReplicateError>> {
        match self.fns(component) {
            Ok(fns) => fns
                .lerp
                .map(|lerp| lerp(from, to, t).map_err(ReplicateError::from)),
            Err(error) => Some(Err(error)),
        }
    }
}

#[cfg(test)]
mod tests;
