//! What there is to work with: components, assets, scenes.

use serde_json::{json, Value};
use voltra_assets::browse;
use voltra_assets::browse::EntryKind;
use voltra_scene::ComponentRegistry;

use super::{ToolError, ToolResult};
use crate::Project;

/// Every component a scene may carry: its name, whether it names other
/// entities, and its template — its default value as RON, the starting
/// point for writing one.
pub fn components_list(registry: &ComponentRegistry) -> ToolResult {
    let mut components = Vec::new();
    for info in registry.describe() {
        let template = match registry.template(info.name) {
            Some(Ok(raw)) => Value::String(raw.get_ron().to_owned()),
            Some(Err(e)) => return Err(ToolError::from_cause(e)),
            None => Value::Null,
        };
        components.push(json!({
            "name": info.name,
            "references_entities": info.has_refs,
            "template": template,
        }));
    }
    Ok(json!({ "components": components }))
}

/// The entries of `dir` under the asset root — the root itself for
/// `None` — with what kind of asset each is.
pub fn assets_list(project: &Project, dir: Option<&str>) -> ToolResult {
    let dir = match dir.filter(|d| !d.trim().is_empty()) {
        Some(raw) => Some(project.resolve(raw).map_err(ToolError::from_cause)?.0),
        None => None,
    };
    let entries = browse::list(project.assets(), dir.as_ref()).map_err(ToolError::from_cause)?;
    let entries: Vec<Value> = entries
        .iter()
        .map(|entry| {
            json!({
                "name": entry.name,
                "path": entry.path.as_str(),
                "kind": format!("{:?}", entry.kind),
            })
        })
        .collect();
    Ok(json!({ "entries": entries }))
}

/// Every scene under the asset root — which is also every prefab — as
/// asset paths, sorted.
pub fn scenes_list(project: &Project) -> ToolResult {
    let mut scenes: Vec<String> = browse::find(project.assets(), EntryKind::Scene)
        .iter()
        .map(|path| path.as_str().to_owned())
        .collect();
    scenes.sort();
    Ok(json!({ "scenes": scenes }))
}
