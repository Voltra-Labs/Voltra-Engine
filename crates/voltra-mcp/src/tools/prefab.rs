//! Placing a prefab in a scene file.

use serde_json::json;
use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::format::{load, save};
use voltra_scene::hierarchy::{resolve_parents, set_parent};
use voltra_scene::{prefab, ComponentRegistry, Prefabs, Transform};

use super::{ToolError, ToolResult};
use crate::Project;
use voltra_scene::edit::{find, id_of};

/// An instance of the prefab at `prefab_path` in the scene at `scene_path`,
/// its root at `at` (and under `parent`, if one is named) — linked to the
/// file as the editor's drop links one, so later edits to the prefab reach
/// it. Answers the new root's id and how many entities it brought.
pub fn prefab_instantiate(
    project: &Project,
    scene_path: &str,
    prefab_path: &str,
    at: Vec2,
    parent: Option<&str>,
) -> ToolResult {
    let (scene_asset, scene_full) = project.resolve(scene_path).map_err(ToolError::from_cause)?;
    let (prefab_asset, prefab_full) = project
        .resolve(prefab_path)
        .map_err(ToolError::from_cause)?;
    if !prefab_full.is_file() {
        return Err(ToolError(format!("{} is not there", prefab_asset.as_str())));
    }
    if scene_asset == prefab_asset {
        return Err(ToolError(
            "a scene cannot hold an instance of itself".to_owned(),
        ));
    }
    let registry = ComponentRegistry::with_defaults();
    let mut world = World::new();
    load(&scene_full, &registry, &mut world)
        .map_err(|e| ToolError(format!("{}: {e}", scene_asset.as_str())))?;
    let parent = parent
        .map(|p| find(&world, p))
        .transpose()
        .map_err(ToolError::from_cause)?;

    let mut prefabs = Prefabs::new(project.assets());
    let handle = prefabs.load(&prefab_asset);
    let source = prefabs.get(handle);
    if source.is_empty() {
        return Err(ToolError(format!(
            "{} would not read as a prefab, or holds nothing",
            prefab_asset.as_str()
        )));
    }
    let instance = prefab::spawn(
        &mut world,
        &registry,
        source,
        &Transform::from_translation(at),
    )
    .map_err(ToolError::from_cause)?;
    let root = instance
        .root()
        .ok_or_else(|| ToolError(format!("{} has no entities", prefab_asset.as_str())))?;
    if let Some(parent) = parent {
        set_parent(&mut world, root, parent).map_err(ToolError::from_cause)?;
    }
    resolve_parents(&mut world);
    save(&world, &registry, &scene_full)
        .map_err(|e| ToolError(format!("{}: {e}", scene_asset.as_str())))?;
    Ok(json!({
        "root": id_of(&world, root),
        "entities": instance.entities.len(),
        "prefab": prefab_asset.as_str(),
    }))
}
