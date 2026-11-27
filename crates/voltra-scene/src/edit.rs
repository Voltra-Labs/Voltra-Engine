//! Edits to a world by name: entities found by id or by a path of names,
//! components set from RON text through the registry.
//!
//! What a tool driving the engine — the MCP server on a file, the editor's
//! bridge on its live scene — does to a world, in one place, so the two
//! answer the same input the same way.

use std::collections::BTreeMap;
use std::fmt;

use ron::value::RawValue;
use voltra_ecs::{Entity, World};

use crate::format::{entity_with_id, record_entity, ComponentRegistry};
use crate::hierarchy::{clear_parent, despawn_recursive, resolve_parents, roots, set_parent};
use crate::keyframes::target::resolve;
use crate::{Name, SceneId};

/// Components not written by name: the parent link is [`reparent`]'s, and
/// the prefab links belong to prefabs.
pub const STRUCTURE: [&str; 3] = [
    crate::Parent::NAME,
    crate::PrefabInstance::NAME,
    crate::PrefabPart::NAME,
];

/// Why an edit was refused, in words for whoever asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditError(pub String);

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for EditError {}

/// The entity `reference` names: a scene id (a UUID), or a path of names
/// from a root — `"Player"`, `"Player/Arm"`. A path two roots answer is
/// refused, naming both ids, rather than guessed at.
pub fn find(world: &World, reference: &str) -> Result<Entity, EditError> {
    let reference = reference.trim();
    if let Ok(uuid) = reference.parse() {
        return entity_with_id(world, SceneId(uuid))
            .ok_or_else(|| EditError(format!("no entity has the id {reference}")));
    }
    let (first, rest) = reference.split_once('/').unwrap_or((reference, ""));
    let matches: Vec<Entity> = roots(world)
        .into_iter()
        .filter(|&root| world.get::<Name>(root).is_some_and(|n| n.as_str() == first))
        .filter_map(|root| resolve(world, root, rest))
        .collect();
    match matches.as_slice() {
        [one] => Ok(*one),
        [] => Err(EditError(format!(
            "no entity is named {reference:?}: give its id, or a path of names from a root"
        ))),
        many => Err(EditError(format!(
            "{} entities are named {reference:?}; give one of their ids: {}",
            many.len(),
            many.iter()
                .map(|&e| id_of(world, e))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// `entity`'s id as text, or empty for one without.
pub fn id_of(world: &World, entity: Entity) -> String {
    world
        .get::<SceneId>(entity)
        .map_or_else(String::new, |id| id.0.to_string())
}

/// Every registered component `entity` carries, as RON by name, with its id.
pub fn components_of(
    world: &World,
    registry: &ComponentRegistry,
    entity: Entity,
) -> Result<(SceneId, BTreeMap<String, String>), EditError> {
    let record = record_entity(world, registry, entity)
        .ok_or_else(|| EditError("the entity has no id".to_owned()))?
        .map_err(|e| EditError(e.to_string()))?;
    let components = record
        .components
        .iter()
        .map(|(name, raw)| (name.clone(), raw.get_ron().to_owned()))
        .collect();
    Ok((record.id, components))
}

/// Sets the component `name` on `entity` from `ron`, through the registry.
pub fn set_component(
    world: &mut World,
    registry: &ComponentRegistry,
    entity: Entity,
    name: &str,
    ron: &str,
) -> Result<(), EditError> {
    if STRUCTURE.contains(&name) {
        return Err(EditError(format!(
            "{name} is structure: reparent for a parent; prefab links are made by prefabs"
        )));
    }
    let raw = RawValue::from_ron(ron).map_err(|e| EditError(format!("{name}: {e}")))?;
    registry
        .load_one(world, entity, name, raw)
        .ok_or_else(|| EditError(format!("no component is registered as {name:?}")))?
        .map_err(|e| EditError(e.to_string()))
}

/// Takes the component `name` off `entity`.
pub fn remove_component(
    world: &mut World,
    registry: &ComponentRegistry,
    entity: Entity,
    name: &str,
) -> Result<(), EditError> {
    if STRUCTURE.contains(&name) {
        return Err(EditError(format!(
            "{name} is structure and is not removed by name"
        )));
    }
    if registry.remove_one(world, entity, name) {
        Ok(())
    } else {
        Err(EditError(format!("no component is registered as {name:?}")))
    }
}

/// A new entity with an id, `name`, the `components` (name → RON) and a
/// place under `parent`. On any failure the entity is despawned again: a
/// world is never left holding half of one.
pub fn spawn<'a>(
    world: &mut World,
    registry: &ComponentRegistry,
    name: Option<&str>,
    parent: Option<Entity>,
    components: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<Entity, EditError> {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    if let Some(name) = name {
        world.insert(entity, Name::new(name));
    }
    let made = components
        .into_iter()
        .try_for_each(|(component, ron)| set_component(world, registry, entity, component, ron))
        .and_then(|()| match parent {
            Some(parent) => reparent(world, entity, Some(parent)),
            None => Ok(()),
        });
    if let Err(e) = made {
        world.despawn(entity);
        return Err(e);
    }
    Ok(entity)
}

/// Puts `entity` under `parent`, or makes it a root for `None`.
pub fn reparent(
    world: &mut World,
    entity: Entity,
    parent: Option<Entity>,
) -> Result<(), EditError> {
    match parent {
        Some(parent) => {
            set_parent(world, entity, parent).map_err(|e| EditError(e.to_string()))?;
            resolve_parents(world);
        }
        None => {
            clear_parent(world, entity);
        }
    }
    Ok(())
}

/// Names `entity` `name`; an empty name takes the name off.
pub fn rename(world: &mut World, entity: Entity, name: &str) {
    if name.is_empty() {
        world.remove::<Name>(entity);
    } else {
        world.insert(entity, Name::new(name));
    }
}

/// Deletes `entity` and everything under it; answers how many went.
pub fn delete(world: &mut World, entity: Entity) -> usize {
    despawn_recursive(world, entity).len().max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_spawn_leaves_nothing_behind_and_a_good_one_is_found_by_name() {
        let registry = ComponentRegistry::with_defaults();
        let mut world = World::new();
        let before = world.query::<SceneId>().count();
        assert!(spawn(
            &mut world,
            &registry,
            Some("Bad"),
            None,
            [("Transform", "(oops)")]
        )
        .is_err());
        assert_eq!(
            world.query::<SceneId>().count(),
            before,
            "nothing left over"
        );

        let player = spawn(
            &mut world,
            &registry,
            Some("Player"),
            None,
            [("PointLight2D", "(radius: 2.0)")],
        )
        .expect("spawns");
        let arm = spawn(&mut world, &registry, Some("Arm"), Some(player), []).expect("spawns");
        assert_eq!(find(&world, "Player/Arm"), Ok(arm));
        assert_eq!(find(&world, &id_of(&world, arm)), Ok(arm));
        spawn(&mut world, &registry, Some("Player"), None, []).expect("a second");
        assert!(find(&world, "Player").is_err_and(|e| e.0.contains("give one of their ids")));
        assert!(set_component(&mut world, &registry, arm, "Parent", "()").is_err());
        assert!(remove_component(&mut world, &registry, arm, "Nope").is_err());
        let (_, components) = components_of(&world, &registry, arm).expect("an id");
        assert!(components.contains_key("Name"));
        assert_eq!(delete(&mut world, player), 2);
    }
}
