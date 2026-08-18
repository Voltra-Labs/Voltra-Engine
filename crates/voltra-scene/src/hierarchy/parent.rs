//! The link a child holds to its parent.

use serde::{Deserialize, Serialize};
use voltra_ecs::Entity;

use crate::scene_id::SceneId;

/// The entity this one is a child of.
///
/// Held on the **child only**. A `Children` list on the parent would be a
/// second copy of the same fact, and every spawn, despawn and reparent would
/// have to keep the two agreeing — Bevy carries both and spends real API
/// surface on that invariant. One direction cannot disagree with itself, and
/// the list a panel wants is one pass over the children ([`children_of`]).
///
/// Stored by identity, resolved to a handle, exactly like [`Sprite`]'s texture:
/// `id` is the truth and the only thing a file carries, `entity` is what this
/// session resolved it to. An `Entity` is an index and a generation, both
/// recycled by the allocator, so writing one to disk would make a scene file
/// mean something different every run.
///
/// [`children_of`]: super::link::children_of
/// [`Sprite`]: crate::Sprite
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Parent {
    /// The parent's stable identity. The only form the file carries.
    pub id: SceneId,
    /// The live entity `id` resolves to, or `None` while it does not resolve.
    ///
    /// Never serialised, and rebuilt by [`resolve_parents`] after anything that
    /// spawns entities: a load, an undo, a play-mode restore. `None` is not an
    /// error — a hand-edited file can name a parent that is not in it, and such
    /// a child draws as a root while keeping the link, so saving writes back the
    /// id it came with rather than quietly dropping it.
    ///
    /// [`resolve_parents`]: super::resolve::resolve_parents
    #[serde(skip)]
    pub entity: Option<Entity>,
}

impl Parent {
    /// An unresolved link to `id`.
    pub fn new(id: SceneId) -> Self {
        Self { id, entity: None }
    }

    /// A link to `id`, already resolved to `entity`.
    pub fn resolved(id: SceneId, entity: Entity) -> Self {
        Self {
            id,
            entity: Some(entity),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_ecs::World;

    #[test]
    fn a_parent_link_round_trips_as_a_bare_id() {
        let id = SceneId::new();
        let text = ron::to_string(&Parent::new(id)).expect("a uuid serializes");

        let back: Parent = ron::from_str(&text).expect("what we just wrote");

        assert_eq!(back.id, id);
    }

    #[test]
    fn the_resolved_entity_never_reaches_the_file() {
        // The whole reason the link is stored by id. A serialized `Entity`
        // would name whatever the allocator handed out the run it was saved.
        let entity = World::new().spawn();
        let link = Parent::resolved(SceneId::new(), entity);

        let text = ron::to_string(&link).expect("a uuid serializes");
        assert!(!text.contains("entity"), "got {text}");

        let back: Parent = ron::from_str(&text).expect("what we just wrote");
        assert_eq!(back.entity, None, "a loaded link starts unresolved");
    }
}
