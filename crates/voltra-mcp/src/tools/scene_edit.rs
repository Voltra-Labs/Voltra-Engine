//! Editing a scene file: entities spawned, deleted, reparented, renamed;
//! components set and removed — each loaded, edited through the registry
//! and saved atomically, or not saved at all.

use ron::value::RawValue;
use serde_json::{json, Map, Value};
use voltra_ecs::{Entity, World};
use voltra_scene::format::{load, record_entity, save, write, SceneFile, VERSION};
use voltra_scene::hierarchy::{clear_parent, despawn_recursive, resolve_parents, set_parent};
use voltra_scene::{ComponentRegistry, Name, SceneId};

use super::entity_ref::{find, id_of};
use super::{ToolError, ToolResult};
use crate::Project;

/// Components a tool must not write by name: the parent link is
/// `entity_reparent`'s, and the prefab links belong to prefabs.
const STRUCTURE: [&str; 3] = ["Parent", "PrefabInstance", "PrefabPart"];

/// Loads `path`, runs `edit`, and saves only if it succeeded.
fn with_scene(
    project: &Project,
    path: &str,
    edit: impl FnOnce(&mut World, &ComponentRegistry) -> ToolResult,
) -> ToolResult {
    let (asset, full) = project.resolve(path).map_err(ToolError::from_cause)?;
    let registry = ComponentRegistry::with_defaults();
    let mut world = World::new();
    load(&full, &registry, &mut world)
        .map_err(|e| ToolError(format!("{}: {e}", asset.as_str())))?;
    let answer = edit(&mut world, &registry)?;
    save(&world, &registry, &full).map_err(|e| ToolError(format!("{}: {e}", asset.as_str())))?;
    Ok(answer)
}

/// `entity` as `scene_read` shows one.
fn describe(world: &World, registry: &ComponentRegistry, entity: Entity) -> ToolResult {
    let record = record_entity(world, registry, entity)
        .ok_or_else(|| ToolError("the entity has no id".to_owned()))?
        .map_err(ToolError::from_cause)?;
    let components: Map<String, Value> = record
        .components
        .iter()
        .map(|(name, raw)| (name.clone(), Value::String(raw.get_ron().to_owned())))
        .collect();
    Ok(json!({ "id": record.id.0.to_string(), "components": components }))
}

/// Sets the component `name` on `entity` from `ron`.
fn set(
    world: &mut World,
    registry: &ComponentRegistry,
    entity: Entity,
    name: &str,
    ron: &str,
) -> Result<(), ToolError> {
    if STRUCTURE.contains(&name) {
        return Err(ToolError(format!(
            "{name} is structure: use entity_reparent for a parent; prefab links are made by prefabs"
        )));
    }
    let raw = RawValue::from_ron(ron).map_err(|e| ToolError(format!("{name}: {e}")))?;
    registry
        .load_one(world, entity, name, raw)
        .ok_or_else(|| {
            ToolError(format!(
                "no component is registered as {name:?}; see components_list"
            ))
        })?
        .map_err(ToolError::from_cause)
}

/// A new, empty scene at `path`. Refused if one is there.
pub fn scene_create(project: &Project, path: &str) -> ToolResult {
    let (asset, full) = project.resolve(path).map_err(ToolError::from_cause)?;
    if !voltra_assets::watch::is_scene(asset.as_str()) {
        return Err(ToolError(format!(
            "{} is not a scene file name (*.ron)",
            asset.as_str()
        )));
    }
    if full.exists() {
        return Err(ToolError(format!("{} already exists", asset.as_str())));
    }
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(ToolError::from_cause)?;
    }
    let file = SceneFile {
        version: VERSION,
        entities: Vec::new(),
    };
    write(&file, &full).map_err(ToolError::from_cause)?;
    Ok(json!({ "created": asset.as_str() }))
}

/// A new entity with `components` (name → RON), optionally named and under
/// `parent`.
pub fn entity_spawn(
    project: &Project,
    path: &str,
    name: Option<&str>,
    parent: Option<&str>,
    components: &Map<String, Value>,
) -> ToolResult {
    with_scene(project, path, |world, registry| {
        let parent = parent.map(|p| find(world, p)).transpose()?;
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        if let Some(name) = name {
            world.insert(entity, Name::new(name));
        }
        for (component, value) in components {
            let ron = value.as_str().ok_or_else(|| {
                ToolError(format!("{component}: a component's value is a RON string"))
            })?;
            set(world, registry, entity, component, ron)?;
        }
        if let Some(parent) = parent {
            set_parent(world, entity, parent).map_err(ToolError::from_cause)?;
            resolve_parents(world);
        }
        describe(world, registry, entity)
    })
}

/// Deletes `entity` and everything under it.
pub fn entity_delete(project: &Project, path: &str, entity: &str) -> ToolResult {
    with_scene(project, path, |world, _| {
        let target = find(world, entity)?;
        let id = id_of(world, target);
        let gone = despawn_recursive(world, target).len().max(1);
        Ok(json!({ "deleted": id, "entities_removed": gone }))
    })
}

/// Puts `entity` under `parent`, or makes it a root for `None`.
pub fn entity_reparent(
    project: &Project,
    path: &str,
    entity: &str,
    parent: Option<&str>,
) -> ToolResult {
    with_scene(project, path, |world, registry| {
        let child = find(world, entity)?;
        match parent {
            Some(parent) => {
                let parent = find(world, parent)?;
                set_parent(world, child, parent).map_err(ToolError::from_cause)?;
                resolve_parents(world);
            }
            None => {
                clear_parent(world, child);
            }
        }
        describe(world, registry, child)
    })
}

/// Names `entity` `name`; an empty name takes the name off.
pub fn entity_rename(project: &Project, path: &str, entity: &str, name: &str) -> ToolResult {
    with_scene(project, path, |world, registry| {
        let target = find(world, entity)?;
        if name.is_empty() {
            world.remove::<Name>(target);
        } else {
            world.insert(target, Name::new(name));
        }
        describe(world, registry, target)
    })
}

/// Sets the component `component` on `entity` from `ron`.
pub fn component_set(
    project: &Project,
    path: &str,
    entity: &str,
    component: &str,
    ron: &str,
) -> ToolResult {
    with_scene(project, path, |world, registry| {
        let target = find(world, entity)?;
        set(world, registry, target, component, ron)?;
        describe(world, registry, target)
    })
}

/// Takes the component `component` off `entity`.
pub fn component_remove(
    project: &Project,
    path: &str,
    entity: &str,
    component: &str,
) -> ToolResult {
    with_scene(project, path, |world, registry| {
        let target = find(world, entity)?;
        if STRUCTURE.contains(&component) {
            return Err(ToolError(format!(
                "{component} is structure and is not removed by name"
            )));
        }
        if !registry.remove_one(world, target, component) {
            return Err(ToolError(format!(
                "no component is registered as {component:?}"
            )));
        }
        describe(world, registry, target)
    })
}
