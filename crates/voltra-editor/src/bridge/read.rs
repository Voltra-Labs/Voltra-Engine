//! The open scene's entities, as the MCP's `scene_read` answers a file.

use serde_json::{json, Value};
use voltra_ecs::World;
use voltra_scene::edit::components_of;
use voltra_scene::hierarchy::parent_of;
use voltra_scene::{ComponentRegistry, Name, SceneId};

/// Every entity with an id: its id, name, parent's id and components as RON.
pub(super) fn entities(world: &World, registry: &ComponentRegistry) -> Result<Value, String> {
    let mut entities = Vec::new();
    for (entity, _) in world.query::<SceneId>() {
        let (id, components) = components_of(world, registry, entity).map_err(|e| e.to_string())?;
        let parent = parent_of(world, entity)
            .and_then(|p| world.get::<SceneId>(p))
            .map(|p| p.0.to_string());
        entities.push(json!({
            "id": id.0.to_string(),
            "name": world.get::<Name>(entity).map(|n| n.as_str().to_owned()),
            "parent": parent,
            "components": components,
        }));
    }
    Ok(json!({ "entities": entities }))
}
