//! A scene file, as entities with their components in RON.

use serde_json::{json, Map, Value};
use voltra_scene::format::SceneFile;
use voltra_scene::{Name, Parent};

use super::{ToolError, ToolResult};
use crate::Project;

/// The scene at `path`: every entity's id, name, parent's id and
/// components, each component as the RON the file holds — what
/// `component_set` takes back.
pub fn scene_read(project: &Project, path: &str) -> ToolResult {
    let (asset, full) = project.resolve(path).map_err(ToolError::from_cause)?;
    let text = std::fs::read_to_string(&full)
        .map_err(|e| ToolError(format!("{}: {e}", asset.as_str())))?;
    let file: SceneFile =
        ron::from_str(&text).map_err(|e| ToolError(format!("{}: {e}", asset.as_str())))?;
    let entities: Vec<Value> = file
        .entities
        .iter()
        .map(|record| {
            let name = record
                .components
                .get(Name::NAME)
                .and_then(|raw| raw.into_rust::<Name>().ok())
                .map(|name| name.as_str().to_owned());
            let parent = record
                .components
                .get(Parent::NAME)
                .and_then(|raw| raw.into_rust::<Parent>().ok())
                .map(|parent| parent.id.0.to_string());
            let components: Map<String, Value> = record
                .components
                .iter()
                .map(|(name, raw)| (name.clone(), Value::String(raw.get_ron().to_owned())))
                .collect();
            json!({
                "id": record.id.0.to_string(),
                "name": name,
                "parent": parent,
                "components": components,
            })
        })
        .collect();
    Ok(json!({
        "path": asset.as_str(),
        "version": file.version,
        "entities": entities,
    }))
}
