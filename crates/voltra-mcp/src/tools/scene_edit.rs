//! Editing a scene file: entities spawned, deleted, reparented, renamed;
//! components set and removed — each loaded, edited through
//! `voltra_scene::edit` and saved atomically, or not saved at all.

use serde_json::{json, Map, Value};
use voltra_ecs::{Entity, World};
use voltra_scene::edit::{self, find};
use voltra_scene::format::{load, save, write, SceneFile, VERSION};
use voltra_scene::ComponentRegistry;

use super::{ToolError, ToolResult};
use crate::Project;

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
    let (id, components) =
        edit::components_of(world, registry, entity).map_err(ToolError::from_cause)?;
    Ok(json!({ "id": id.0.to_string(), "components": components }))
}

fn found(world: &World, reference: &str) -> Result<Entity, ToolError> {
    find(world, reference).map_err(ToolError::from_cause)
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
        let parent = parent.map(|p| found(world, p)).transpose()?;
        let mut pairs = Vec::with_capacity(components.len());
        for (component, value) in components {
            let ron = value.as_str().ok_or_else(|| {
                ToolError(format!("{component}: a component's value is a RON string"))
            })?;
            pairs.push((component.as_str(), ron));
        }
        let entity =
            edit::spawn(world, registry, name, parent, pairs).map_err(ToolError::from_cause)?;
        describe(world, registry, entity)
    })
}

/// Deletes `entity` and everything under it.
pub fn entity_delete(project: &Project, path: &str, entity: &str) -> ToolResult {
    with_scene(project, path, |world, _| {
        let target = found(world, entity)?;
        let id = edit::id_of(world, target);
        let gone = edit::delete(world, target);
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
        let child = found(world, entity)?;
        let parent = parent.map(|p| found(world, p)).transpose()?;
        edit::reparent(world, child, parent).map_err(ToolError::from_cause)?;
        describe(world, registry, child)
    })
}

/// Names `entity` `name`; an empty name takes the name off.
pub fn entity_rename(project: &Project, path: &str, entity: &str, name: &str) -> ToolResult {
    with_scene(project, path, |world, registry| {
        let target = found(world, entity)?;
        edit::rename(world, target, name);
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
        let target = found(world, entity)?;
        edit::set_component(world, registry, target, component, ron)
            .map_err(ToolError::from_cause)?;
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
        let target = found(world, entity)?;
        edit::remove_component(world, registry, target, component)
            .map_err(ToolError::from_cause)?;
        describe(world, registry, target)
    })
}
