//! Naming an entity in a tool call: by its id, or by the names from a root
//! down.

use voltra_ecs::{Entity, World};
use voltra_scene::format::entity_with_id;
use voltra_scene::hierarchy::roots;
use voltra_scene::keyframes::target::resolve;
use voltra_scene::{Name, SceneId};

use super::ToolError;

/// The entity `reference` names in `world`: a scene id (the UUID a scene
/// file and `scene_read` give), or a path of names from a root — `"Player"`,
/// `"Player/Arm"`. A path two roots answer is refused, naming both ids, so
/// the client picks one rather than the tool guessing.
pub fn find(world: &World, reference: &str) -> Result<Entity, ToolError> {
    let reference = reference.trim();
    if let Ok(uuid) = reference.parse() {
        return entity_with_id(world, SceneId(uuid))
            .ok_or_else(|| ToolError(format!("no entity has the id {reference}")));
    }
    let (first, rest) = reference.split_once('/').unwrap_or((reference, ""));
    let matches: Vec<Entity> = roots(world)
        .into_iter()
        .filter(|&root| world.get::<Name>(root).is_some_and(|n| n.as_str() == first))
        .filter_map(|root| resolve(world, root, rest))
        .collect();
    match matches.as_slice() {
        [one] => Ok(*one),
        [] => Err(ToolError(format!(
            "no entity is named {reference:?}: give its id, or a path of names from a root"
        ))),
        many => Err(ToolError(format!(
            "{} entities are named {reference:?}; give one of their ids: {}",
            many.len(),
            many.iter()
                .filter_map(|&e| world.get::<SceneId>(e).map(|id| id.0.to_string()))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// `entity`'s id, as tools answer it.
pub fn id_of(world: &World, entity: Entity) -> String {
    world
        .get::<SceneId>(entity)
        .map_or_else(String::new, |id| id.0.to_string())
}
