//! What the registry keeps per type: its name and the functions that know
//! it.

use ron::value::RawValue;
use voltra_ecs::{Entity, World};

use crate::format::error::SceneError;
use crate::format::serial_tree::SerialTree;
use crate::scene_id::SceneId;

/// A registered type's save conversion: entity to stored value, or `None` when
/// the entity has no such component.
pub(super) type SaveFn = fn(&World, Entity) -> Option<Result<Box<RawValue>, SceneError>>;

/// A registered type's load conversion: stored value to entity.
///
/// Returns the bare `ron` error rather than a [`SceneError`]: this function
/// only knows `T`, not the registered name the [`Entry`] was stored under, and
/// `SceneError::Component` needs that name to point at anything a user can act
/// on. `load_one` has the `Entry` in hand, so it wraps this into a `SceneError`
/// with the real name instead of `T`'s Rust path.
pub(super) type LoadFn = fn(&mut World, Entity, &RawValue) -> Result<(), ron::error::SpannedError>;

/// A registered type's removal.
///
/// Needed because a record describes an entity *exactly*: putting one back has
/// to take off the components it does not carry, not only put on the ones it
/// does. Without this, undoing the addition of a component leaves it behind and
/// reports success.
pub(super) type RemoveFn = fn(&mut World, Entity);

/// Whether an entity has a registered type, without writing it out: what a
/// search by component asks of every entity in a scene, every frame.
pub(super) type HasFn = fn(&World, Entity) -> bool;

/// A registered type's round trip: a stored value read as the type and
/// written back, for comparing values whatever text they were stored as.
pub(super) type NormalizeFn = fn(&RawValue) -> Result<Box<RawValue>, SceneError>;

/// A registered type's value on an entity as a [`SerialTree`], or `None`
/// when the entity has none: what a tool compares and writes field by
/// field.
pub(super) type TreeFn = fn(&World, Entity) -> Option<Result<SerialTree, SceneError>>;

/// A registered type's references rewritten on an entity, for a type that
/// names other entities — see [`super::refs`].
pub(super) type RemapFn = fn(&mut World, Entity, &dyn Fn(SceneId) -> Option<SceneId>);

/// The same, for a stored value.
pub(super) type RewriteFn =
    fn(&RawValue, &dyn Fn(SceneId) -> Option<SceneId>) -> Result<Box<RawValue>, SceneError>;

/// A registered type's default value, written as it would be saved: what
/// a tool fills in to make one.
pub(super) type TemplateFn = fn() -> Result<Box<RawValue>, SceneError>;

/// One component type's name and the conversions that go with it.
///
/// `save`/`load`/`remove` are read by `save_one`/`load_one`/`remove_one` below.
#[derive(Clone)]
pub(super) struct Entry {
    pub(super) name: &'static str,
    pub(super) save: SaveFn,
    pub(super) load: LoadFn,
    pub(super) remove: RemoveFn,
    pub(super) has: HasFn,
    pub(super) normalize: NormalizeFn,
    pub(super) tree: TreeFn,
    /// For a type naming other entities by id; `None` for every other.
    pub(super) refs: Option<(RemapFn, RewriteFn)>,
    /// For a type with a meaningful default; `None` for one without, or
    /// for structure (a parent link, a prefab part) a tool must not make
    /// up.
    pub(super) template: Option<TemplateFn>,
    /// The type itself, so [`ComponentRegistry::replicate`] can find the
    /// entry a `T` was registered under.
    ///
    /// [`ComponentRegistry::replicate`]: super::ComponentRegistry::replicate
    pub(super) type_id: std::any::TypeId,
    /// For a type the server sends its clients; `None` for every other.
    pub(super) replicate: Option<super::replicate::ReplicateFns>,
}
