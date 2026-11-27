//! Prefab mode over the bridge: into a prefab, back to the scene, and a
//! variant of one.

use serde_json::{json, Value};
use voltra_assets::AssetPath;
use voltra_bridge::Command;
use voltra_core::UiFrame;
use voltra_scene::SceneId;

use crate::editor::Editor;
use crate::prefab_mode;

/// Applies `prefab_open`, `prefab_close` or `prefab_variant`.
pub(super) fn apply(
    editor: &mut Editor,
    frame: &mut UiFrame<'_>,
    command: &Command,
) -> Result<Value, String> {
    match command {
        Command::PrefabOpen { path, entity } => {
            let path = match (path, entity) {
                (Some(path), None) => AssetPath::new(path).map_err(|e| e.to_string())?,
                (None, Some(reference)) => {
                    let entity = voltra_scene::edit::find(frame.world, reference)
                        .map_err(|e| e.to_string())?;
                    prefab_mode::source_of(frame.world, entity)
                        .ok_or_else(|| format!("{reference} is not part of a prefab instance"))?
                }
                _ => {
                    return Err(
                        "name the prefab's path or an instance's entity, one of the two".into(),
                    )
                }
            };
            prefab_mode::enter(editor, frame, &path)?;
            Ok(json!({
                "editing": path.as_str(),
                "from": editor.prefab_mode.returns_to(),
                "entities": frame.world.query::<SceneId>().count(),
            }))
        }
        Command::PrefabClose { discard_changes } => {
            if !editor.prefab_mode.is_active() {
                return Err("no prefab is open".into());
            }
            super::unsaved::keep_unsaved(editor, *discard_changes)?;
            let left = prefab_mode::leave(editor, frame)?;
            Ok(json!({
                "left": left.as_str(),
                "scene": editor.document.path().map(AssetPath::as_str),
                "prefab_mode": editor.prefab_mode.is_active(),
                "entities": frame.world.query::<SceneId>().count(),
            }))
        }
        Command::PrefabVariant { path } => {
            let base = AssetPath::new(path).map_err(|e| e.to_string())?;
            let variant = crate::prefab::create_variant(editor, frame, &base)?;
            Ok(json!({ "variant": variant.as_str(), "base": base.as_str() }))
        }
        _ => Err("not a prefab mode command".into()),
    }
}
