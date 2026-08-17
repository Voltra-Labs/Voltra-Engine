//! One undoable entry, and putting either of its sides back.
//!
//! An entry is a set of per-entity records with an `Option` on each side, which
//! is what lets one type describe a modification, a spawn, a delete and a
//! scene-wide clear with no code per action:
//!
//! | Action | `before` | `after` |
//! | --- | --- | --- |
//! | Move, edit a field | `Some` | `Some` |
//! | Spawn | `None` | `Some` |
//! | Delete, Clear | `Some` | `None` |
//!
//! Entities are addressed by [`SceneId`] and never by `Entity`: an undone
//! delete has to revive the entity the rest of the stack still refers to, and
//! the allocator recycles both the index and the generation.

use voltra_ecs::{Entity, World};
use voltra_scene::format::{apply_record, entity_with_id, EntityRecord};
use voltra_scene::{ComponentRegistry, SceneError, SceneId};

/// Which side of an edit to put back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Before,
    After,
}

/// One entity's state on both sides of an edit.
///
/// `None` means the entity did not exist on that side.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityChange {
    pub id: SceneId,
    pub before: Option<EntityRecord>,
    pub after: Option<EntityRecord>,
}

impl EntityChange {
    fn side(&self, side: Side) -> Option<&EntityRecord> {
        match side {
            Side::Before => self.before.as_ref(),
            Side::After => self.after.as_ref(),
        }
    }
}

/// One entry in the history.
#[derive(Debug, Clone, PartialEq)]
pub struct Edit {
    /// What the Edit menu calls it.
    ///
    /// `&'static str` rather than `String`: every label is written in the
    /// source, and a history is the wrong place to keep a hundred identical
    /// heap allocations.
    pub label: &'static str,
    pub entities: Vec<EntityChange>,
    pub selected_before: Option<SceneId>,
    pub selected_after: Option<SceneId>,
}

impl Edit {
    /// Whether the two sides describe the same scene.
    ///
    /// A grab that pressed a handle and released without moving, a text field
    /// clicked into and out of: both produce an edit, and neither is one.
    pub fn is_noop(&self) -> bool {
        self.entities
            .iter()
            .all(|change| change.before == change.after)
    }

    /// What was selected on `side`.
    pub fn selection(&self, side: Side) -> Option<SceneId> {
        match side {
            Side::Before => self.selected_before,
            Side::After => self.selected_after,
        }
    }

    /// The entity `side`'s selection names, if the world holds it.
    ///
    /// Resolved after the apply, never before: the entity that carries the id
    /// may be one the apply has just respawned.
    pub fn selected_entity(&self, side: Side, world: &World) -> Option<Entity> {
        entity_with_id(world, self.selection(side)?)
    }

    /// Puts `side` back.
    ///
    /// Every change is applied even after one fails, and the first error is
    /// returned at the end. Stopping at the first would leave the scene half on
    /// one side of the edit and half on the other, which is worse than either
    /// side and impossible to undo out of.
    pub fn apply(
        &self,
        side: Side,
        world: &mut World,
        registry: &ComponentRegistry,
    ) -> Result<(), SceneError> {
        let mut first_error = None;

        for change in &self.entities {
            match change.side(side) {
                Some(record) => {
                    if let Err(error) = apply_record(world, registry, record) {
                        first_error.get_or_insert(error);
                    }
                }
                // Absent on this side: the entity does not exist there. Already
                // gone is the postcondition, so an id nothing carries is not a
                // failure — it is what applying the same side twice looks like.
                None => {
                    if let Some(entity) = entity_with_id(world, change.id) {
                        world.despawn(entity);
                    }
                }
            }
        }

        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_render::glam::Vec2;
    use voltra_scene::format::record_entity;
    use voltra_scene::{Sprite, Transform};

    /// A world holding one sprite at `x`, and the record of it.
    fn one_sprite(x: f32) -> (World, Entity, SceneId, EntityRecord) {
        let registry = ComponentRegistry::with_defaults();
        let mut world = World::new();
        let entity = world.spawn();
        let id = SceneId::new();
        world.insert(entity, id);
        world.insert(entity, Transform::from_translation(Vec2::new(x, 0.0)));
        world.insert(entity, Sprite::default());
        let record = record_entity(&world, &registry, entity)
            .expect("has an id")
            .expect("serializes");
        (world, entity, id, record)
    }

    fn x_of(world: &World, id: SceneId) -> Option<f32> {
        let entity = entity_with_id(world, id)?;
        Some(world.get::<Transform>(entity)?.translation.x)
    }

    #[test]
    fn a_modification_applies_in_both_directions() {
        let registry = ComponentRegistry::with_defaults();
        let (mut world, entity, id, before) = one_sprite(1.0);
        world
            .get_mut::<Transform>(entity)
            .expect("it has one")
            .translation = Vec2::new(5.0, 0.0);
        let after = record_entity(&world, &registry, entity)
            .expect("has an id")
            .expect("serializes");

        let edit = Edit {
            label: "Move",
            entities: vec![EntityChange {
                id,
                before: Some(before),
                after: Some(after),
            }],
            selected_before: Some(id),
            selected_after: Some(id),
        };

        edit.apply(Side::Before, &mut world, &registry)
            .expect("apply");
        assert_eq!(x_of(&world, id), Some(1.0));
        edit.apply(Side::After, &mut world, &registry)
            .expect("apply");
        assert_eq!(x_of(&world, id), Some(5.0));
    }

    #[test]
    fn a_spawn_undoes_to_nothing_and_redoes_back() {
        let registry = ComponentRegistry::with_defaults();
        let (mut world, _, id, after) = one_sprite(2.0);

        let edit = Edit {
            label: "Spawn sprite",
            entities: vec![EntityChange {
                id,
                before: None,
                after: Some(after),
            }],
            selected_before: None,
            selected_after: Some(id),
        };

        edit.apply(Side::Before, &mut world, &registry)
            .expect("apply");
        assert_eq!(world.entity_count(), 0, "undoing a spawn despawns it");
        edit.apply(Side::After, &mut world, &registry)
            .expect("apply");
        assert_eq!(
            x_of(&world, id),
            Some(2.0),
            "redo brings it back with its data"
        );
    }

    #[test]
    fn a_delete_undoes_by_reviving_the_same_scene_id() {
        let registry = ComponentRegistry::with_defaults();
        let (mut world, entity, id, before) = one_sprite(3.0);
        world.despawn(entity);

        let edit = Edit {
            label: "Delete",
            entities: vec![EntityChange {
                id,
                before: Some(before),
                after: None,
            }],
            selected_before: Some(id),
            selected_after: None,
        };

        edit.apply(Side::Before, &mut world, &registry)
            .expect("apply");
        assert_eq!(x_of(&world, id), Some(3.0));
        edit.apply(Side::After, &mut world, &registry)
            .expect("apply");
        assert_eq!(x_of(&world, id), None);
    }

    #[test]
    fn applying_a_side_twice_changes_nothing_the_second_time() {
        // Undo is not a toggle: pressing it twice on the same entry must not
        // spawn a duplicate of the entity it revived.
        let registry = ComponentRegistry::with_defaults();
        let (mut world, entity, id, before) = one_sprite(4.0);
        world.despawn(entity);
        let edit = Edit {
            label: "Delete",
            entities: vec![EntityChange {
                id,
                before: Some(before),
                after: None,
            }],
            selected_before: Some(id),
            selected_after: None,
        };

        edit.apply(Side::Before, &mut world, &registry)
            .expect("apply");
        edit.apply(Side::Before, &mut world, &registry)
            .expect("apply");

        assert_eq!(world.entity_count(), 1);
    }

    #[test]
    fn an_edit_whose_sides_match_is_a_noop() {
        let (_, _, id, record) = one_sprite(1.0);
        let edit = Edit {
            label: "Move",
            entities: vec![EntityChange {
                id,
                before: Some(record.clone()),
                after: Some(record),
            }],
            selected_before: Some(id),
            selected_after: Some(id),
        };
        assert!(
            edit.is_noop(),
            "a grab that moved nothing must not be an entry"
        );
    }

    #[test]
    fn an_edit_with_one_changed_entity_is_not_a_noop() {
        let registry = ComponentRegistry::with_defaults();
        let (mut world, entity, id, before) = one_sprite(1.0);
        world
            .get_mut::<Transform>(entity)
            .expect("it has one")
            .translation = Vec2::new(7.0, 0.0);
        let after = record_entity(&world, &registry, entity)
            .expect("has an id")
            .expect("serializes");

        let edit = Edit {
            label: "Move",
            entities: vec![EntityChange {
                id,
                before: Some(before),
                after: Some(after),
            }],
            selected_before: Some(id),
            selected_after: Some(id),
        };
        assert!(!edit.is_noop());
    }

    #[test]
    fn the_selection_comes_from_the_side_being_applied() {
        let (_, _, id, record) = one_sprite(1.0);
        let edit = Edit {
            label: "Delete",
            entities: vec![EntityChange {
                id,
                before: Some(record),
                after: None,
            }],
            selected_before: Some(id),
            selected_after: None,
        };
        assert_eq!(edit.selection(Side::Before), Some(id));
        assert_eq!(edit.selection(Side::After), None);
    }

    #[test]
    fn the_selected_entity_is_resolved_against_the_world_after_the_apply() {
        // The handle the selection had before an undo is dead: the apply
        // respawned the entity. Only the id survives, and this is where it is
        // turned back into something the editor can select.
        let registry = ComponentRegistry::with_defaults();
        let (mut world, entity, id, before) = one_sprite(3.0);
        world.despawn(entity);

        let edit = Edit {
            label: "Delete",
            entities: vec![EntityChange {
                id,
                before: Some(before),
                after: None,
            }],
            selected_before: Some(id),
            selected_after: None,
        };
        edit.apply(Side::Before, &mut world, &registry)
            .expect("apply");

        let revived = edit.selected_entity(Side::Before, &world);
        assert_eq!(revived, entity_with_id(&world, id));
        assert_ne!(revived, Some(entity), "the old handle is not reused");
    }
}
