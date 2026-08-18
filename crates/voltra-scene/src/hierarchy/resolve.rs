//! Turning stored parent identities back into live entities.

use std::collections::HashMap;

use voltra_ecs::{Entity, World};

use super::parent::Parent;
use crate::scene_id::SceneId;
use crate::{Collider, RigidBody};

/// Points every [`Parent`] link at the entity its [`SceneId`] names.
///
/// Called after anything that spawns entities from stored records — a load, an
/// undo, a play-mode restore — because a record carries the identity and the
/// entity it resolved to last time is gone. This is the same job
/// [`Sprite::set_texture`] does for a texture path, and for the same reason.
///
/// One pass, not a lookup per link: [`entity_with_id`] is linear, and a scene
/// where every entity is parented would make resolving quadratic in the number
/// of entities. Both a load and an undo call this on every applied record.
///
/// A link naming an id the world does not hold is left unresolved rather than
/// dropped: the child draws as a root, and the id it came with is still there
/// to be written back on the next save.
///
/// [`Sprite::set_texture`]: crate::Sprite::set_texture
/// [`entity_with_id`]: crate::format::entity_with_id
pub fn resolve_parents(world: &mut World) {
    let by_id: HashMap<SceneId, Entity> = world
        .query::<SceneId>()
        .map(|(entity, id)| (*id, entity))
        .collect();

    // Collected first: the resolution writes to the same storage the query
    // above reads, and the borrow cannot span both.
    let links: Vec<(Entity, SceneId)> = world
        .query::<Parent>()
        .map(|(entity, link)| (entity, link.id))
        .collect();

    for (entity, id) in links {
        // `set_parent` refuses this combination, so it can only arrive from a
        // file. Warned rather than repaired: dropping the link or the body
        // would both be edits to someone's scene, made without asking, on the
        // strength of a rule this build happens to have today.
        if world.get::<RigidBody>(entity).is_some() || world.get::<Collider>(entity).is_some() {
            log::warn!(
                "{entity:?} is parented and takes part in physics; the solver reads its transform as world space"
            );
        }

        let resolved = by_id.get(&id).copied();
        if resolved.is_none() {
            log::warn!(
                "entity {entity:?} names a parent that is not in the scene; drawing it as a root"
            );
        }
        if let Some(link) = world.get_mut::<Parent>(entity) {
            link.entity = resolved;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy::link::parent_of;

    fn spawn_with_id(world: &mut World) -> (Entity, SceneId) {
        let entity = world.spawn();
        let id = SceneId::new();
        world.insert(entity, id);
        (entity, id)
    }

    #[test]
    fn a_loaded_link_resolves_to_the_entity_holding_the_id() {
        let mut world = World::new();
        let (parent, parent_id) = spawn_with_id(&mut world);
        let (child, _) = spawn_with_id(&mut world);
        // As a load leaves it: the id is there, the entity is not.
        world.insert(child, Parent::new(parent_id));

        resolve_parents(&mut world);

        assert_eq!(parent_of(&world, child), Some(parent));
    }

    #[test]
    fn a_forward_reference_resolves_too() {
        // A file is a list, and a child can appear before its parent. Resolving
        // as each entity is spawned would leave this one unresolved forever.
        let mut world = World::new();
        let parent_id = SceneId::new();
        let (child, _) = spawn_with_id(&mut world);
        world.insert(child, Parent::new(parent_id));
        let parent = world.spawn();
        world.insert(parent, parent_id);

        resolve_parents(&mut world);

        assert_eq!(parent_of(&world, child), Some(parent));
    }

    #[test]
    fn a_link_to_an_absent_parent_stays_unresolved_and_is_kept() {
        let mut world = World::new();
        let (child, _) = spawn_with_id(&mut world);
        let missing = SceneId::new();
        world.insert(child, Parent::new(missing));

        resolve_parents(&mut world);

        assert_eq!(parent_of(&world, child), None);
        assert_eq!(
            world.get::<Parent>(child).map(|link| link.id),
            Some(missing),
            "the id survives, so the next save writes it back"
        );
    }

    #[test]
    fn a_stale_entity_is_replaced_rather_than_trusted() {
        // What an undo produces: the record's id is right, and the `Entity` it
        // resolved to before the despawn is not.
        let mut world = World::new();
        let (parent, parent_id) = spawn_with_id(&mut world);
        let (child, _) = spawn_with_id(&mut world);
        let stale = world.spawn();
        world.insert(child, Parent::resolved(parent_id, stale));

        resolve_parents(&mut world);

        assert_eq!(parent_of(&world, child), Some(parent));
    }

    #[test]
    fn a_parented_body_from_a_file_is_kept_as_it_is() {
        // Warned, not repaired. Deleting either component would be an edit to
        // someone's scene made without asking.
        let mut world = World::new();
        let (parent, parent_id) = spawn_with_id(&mut world);
        let (child, _) = spawn_with_id(&mut world);
        world.insert(child, Parent::new(parent_id));
        world.insert(child, crate::RigidBody::new_dynamic(1.0));

        resolve_parents(&mut world);

        assert_eq!(parent_of(&world, child), Some(parent));
        assert!(world.get::<crate::RigidBody>(child).is_some());
    }

    #[test]
    fn resolving_an_empty_world_does_nothing() {
        let mut world = World::new();
        resolve_parents(&mut world);
        assert_eq!(world.entity_count(), 0);
    }
}
