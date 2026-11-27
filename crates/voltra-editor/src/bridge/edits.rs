//! Edits from the bridge, each one undo entry.

use serde_json::{json, Value};
use voltra_bridge::Command;
use voltra_ecs::{Entity, World};
use voltra_scene::edit::{self, find};
use voltra_scene::format::entity_with_id;
use voltra_scene::hierarchy::descendants;
use voltra_scene::{prefab, PrefabPart, Prefabs, SceneId};

use crate::editor::Editor;
use crate::undo::SceneView;

/// Applies an edit command to `world` as one undo entry named by the
/// command, and answers what the entity is now. A refused edit records
/// nothing.
///
/// An edit to a part of a prefab instance records what that now overrides
/// in the same entry, as the inspector's edits do on the selection — the
/// bridge edits what it names, selected or not.
pub(crate) fn apply(
    editor: &mut Editor,
    world: &mut World,
    prefabs: &mut Prefabs,
    command: &Command,
) -> Result<Value, String> {
    let label = command.label();
    // Which ids the edit may change, known before it runs: the target and,
    // for a delete, everything under it. A spawn has none yet.
    let touched: Vec<SceneId> = match command {
        Command::Spawn { .. } => Vec::new(),
        Command::Delete { entity }
        | Command::Drag { entity, .. }
        | Command::Reparent { entity, .. }
        | Command::Rename { entity, .. }
        | Command::ComponentSet { entity, .. }
        | Command::ComponentRemove { entity, .. } => {
            let target = find(world, entity).map_err(|e| e.to_string())?;
            std::iter::once(target)
                .chain(descendants(world, target))
                .filter_map(|e| world.get::<SceneId>(e).copied())
                .collect()
        }
        _ => return Err("not an edit".into()),
    };
    let view = SceneView {
        world,
        registry: &editor.registry,
        selected: editor.selection.ids(world),
    };
    editor.history.begin(label, view, touched.clone());

    let outcome = run(editor, world, command);
    let made: Vec<SceneId> = match &outcome {
        Ok((Some(entity), _)) => world.get::<SceneId>(*entity).copied().into_iter().collect(),
        _ => Vec::new(),
    };
    let parts: Vec<Entity> = touched
        .iter()
        .chain(&made)
        .filter_map(|&id| entity_with_id(world, id))
        .filter(|&e| world.get::<PrefabPart>(e).is_some())
        .collect();
    for entity in parts {
        if let Err(error) = prefab::refresh_overrides(world, &editor.registry, prefabs, entity) {
            log::error!("could not compare an instance with its prefab: {error}");
        }
    }
    let view = SceneView {
        world,
        registry: &editor.registry,
        selected: editor.selection.ids(world),
    };
    // A refused edit changed nothing; committing it records a no-op the
    // history drops.
    editor.history.commit_including(view, made);
    outcome.map(|(_, answer)| answer)
}

/// The edit itself: the entity it leaves (if any), and the answer.
fn run(
    editor: &Editor,
    world: &mut World,
    command: &Command,
) -> Result<(Option<Entity>, Value), String> {
    let registry = &editor.registry;
    let describe = |world: &World, entity: Entity| -> Result<Value, String> {
        let (id, components) =
            edit::components_of(world, registry, entity).map_err(|e| e.to_string())?;
        Ok(json!({ "id": id.0.to_string(), "components": components }))
    };
    match command {
        Command::Spawn {
            name,
            parent,
            components,
        } => {
            let parent = parent
                .as_deref()
                .map(|p| find(world, p))
                .transpose()
                .map_err(|e| e.to_string())?;
            let entity = edit::spawn(
                world,
                registry,
                name.as_deref(),
                parent,
                components.iter().map(|(k, v)| (k.as_str(), v.as_str())),
            )
            .map_err(|e| e.to_string())?;
            Ok((Some(entity), describe(world, entity)?))
        }
        Command::Delete { entity } => {
            let target = find(world, entity).map_err(|e| e.to_string())?;
            let id = edit::id_of(world, target);
            let gone = edit::delete(world, target);
            Ok((None, json!({ "deleted": id, "entities_removed": gone })))
        }
        Command::Reparent { entity, parent } => {
            let target = find(world, entity).map_err(|e| e.to_string())?;
            let parent = parent
                .as_deref()
                .map(|p| find(world, p))
                .transpose()
                .map_err(|e| e.to_string())?;
            edit::reparent(world, target, parent).map_err(|e| e.to_string())?;
            Ok((Some(target), describe(world, target)?))
        }
        Command::Rename { entity, name } => {
            let target = find(world, entity).map_err(|e| e.to_string())?;
            edit::rename(world, target, name);
            Ok((Some(target), describe(world, target)?))
        }
        Command::ComponentSet {
            entity,
            component,
            ron,
        } => {
            let target = find(world, entity).map_err(|e| e.to_string())?;
            edit::set_component(world, registry, target, component, ron)
                .map_err(|e| e.to_string())?;
            Ok((Some(target), describe(world, target)?))
        }
        Command::ComponentRemove { entity, component } => {
            let target = find(world, entity).map_err(|e| e.to_string())?;
            edit::remove_component(world, registry, target, component)
                .map_err(|e| e.to_string())?;
            Ok((Some(target), describe(world, target)?))
        }
        Command::Drag {
            entity,
            tool,
            from,
            to,
        } => {
            let target = find(world, entity).map_err(|e| e.to_string())?;
            let tool = match tool.as_str() {
                "move" => crate::tool::Tool::Translate,
                "rotate" => crate::tool::Tool::Rotate,
                "scale" => crate::tool::Tool::Scale,
                other => return Err(format!("{other:?} is not move, rotate or scale")),
            };
            if world.get::<voltra_scene::Transform2D>(target).is_none() {
                return Err("it has no transform to drag".into());
            }
            let start = voltra_scene::hierarchy::world_transform_2d(world, target);
            let updated = crate::gizmo::drag_to(
                target,
                tool,
                start,
                (*from).into(),
                (*to).into(),
                &editor.snap,
            );
            if !voltra_scene::hierarchy::set_world_transform_2d(world, target, &updated) {
                return Err("its transform could not be set".into());
            }
            Ok((Some(target), describe(world, target)?))
        }
        _ => Err("not an edit".into()),
    }
}

#[cfg(test)]
mod tests;
