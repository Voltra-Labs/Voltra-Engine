//! Reading and writing asset files — RON ones written only once they read
//! as what they are.

use serde_json::json;
use voltra_assets::atlas::{is_atlas, AtlasFile};
use voltra_assets::browse::EntryKind;
use voltra_assets::watch::{is_animation, is_animator, is_input_map, is_scene, is_sorting_layers};
use voltra_scene::{AnimationClip, Prefab, StateMachine};

use super::{ToolError, ToolResult};
use crate::Project;

/// The most bytes `asset_read` hands back as text.
const MAX_READ: u64 = 1 << 20;

/// What kind of file `name` is, as the asset browser says.
fn kind_of(name: &str) -> EntryKind {
    if is_atlas(name) {
        EntryKind::Atlas
    } else if is_input_map(name) {
        EntryKind::InputMap
    } else if is_sorting_layers(name) {
        EntryKind::SortingLayers
    } else if is_animation(name) {
        EntryKind::Animation
    } else if is_animator(name) {
        EntryKind::Animator
    } else if is_scene(name) {
        EntryKind::Scene
    } else {
        EntryKind::Other
    }
}

/// The text of a RON (or other text) asset; for a binary one — a texture, a
/// sound, a font — its kind and size instead.
pub fn asset_read(project: &Project, path: &str) -> ToolResult {
    let (asset, full) = project.resolve(path).map_err(ToolError::from_cause)?;
    let meta =
        std::fs::metadata(&full).map_err(|e| ToolError(format!("{}: {e}", asset.as_str())))?;
    if meta.is_dir() {
        return Err(ToolError(format!(
            "{} is a directory; see assets_list",
            asset.as_str()
        )));
    }
    let text_like = asset.as_str().ends_with(".ron")
        || asset.as_str().ends_with(".txt")
        || asset.as_str().ends_with(".wgsl");
    if !text_like {
        return Ok(json!({ "path": asset.as_str(), "binary": true, "bytes": meta.len() }));
    }
    if meta.len() > MAX_READ {
        return Err(ToolError(format!(
            "{} is {} bytes; at most {MAX_READ} are read",
            asset.as_str(),
            meta.len()
        )));
    }
    let text = std::fs::read_to_string(&full)
        .map_err(|e| ToolError(format!("{}: {e}", asset.as_str())))?;
    Ok(
        json!({ "path": asset.as_str(), "kind": format!("{:?}", kind_of(asset.as_str())), "text": text }),
    )
}

/// Whether `text` reads as the asset `name` names, with the reason it does
/// not.
fn check(name: &str, text: &str) -> Result<EntryKind, ToolError> {
    let kind = kind_of(name);
    let fail =
        |e: &dyn std::fmt::Display| ToolError(format!("{name} does not read as {kind:?}: {e}"));
    match kind {
        EntryKind::Scene => {
            // Any version the engine reads: an older one is migrated as
            // the player loads it.
            let file = voltra_scene::format::parse(text).map_err(|e| fail(&e))?;
            Prefab::new(file).map_err(|e| fail(&e))?;
        }
        EntryKind::Animation => {
            ron::from_str::<AnimationClip>(text).map_err(|e| fail(&e))?;
        }
        EntryKind::Animator => {
            ron::from_str::<StateMachine>(text).map_err(|e| fail(&e))?;
        }
        EntryKind::InputMap => {
            voltra_core::input::ActionMap::from_ron(text, std::path::Path::new(name)).map_err(|e| fail(&e))?;
        }
        EntryKind::Atlas => {
            let file: AtlasFile = ron::from_str(text).map_err(|e| fail(&e))?;
            file.into_atlas().map_err(|e| fail(&e))?;
        }
        EntryKind::SortingLayers => {
            ron::from_str::<ron::Value>(text).map_err(|e| fail(&e))?;
        }
        _ => {
            return Err(ToolError(format!(
                "{name}: only RON asset files are written (scenes, clips, state machines, input maps, atlases, sorting layers)"
            )))
        }
    }
    Ok(kind)
}

/// Writes `text` to the RON asset at `path`, once it reads as that kind of
/// asset — atomically, so a reader never sees half a file. Refuses to
/// replace an existing file unless `overwrite` says so.
pub fn asset_write(project: &Project, path: &str, text: &str, overwrite: bool) -> ToolResult {
    let (asset, full) = project.resolve(path).map_err(ToolError::from_cause)?;
    let kind = check(asset.as_str(), text)?;
    if full.exists() && !overwrite {
        return Err(ToolError(format!(
            "{} exists; pass overwrite to replace it",
            asset.as_str()
        )));
    }
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(ToolError::from_cause)?;
    }
    voltra_scene::format::atomic::replace(&full, text.as_bytes()).map_err(ToolError::from_cause)?;
    Ok(json!({ "written": asset.as_str(), "kind": format!("{kind:?}") }))
}
