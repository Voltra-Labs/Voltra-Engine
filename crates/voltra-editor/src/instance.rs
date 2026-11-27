//! A prefab instance edited in the editor: what it overrides kept up to
//! date as it is edited, and taken back to its prefab or written over it —
//! each one undo entry. See `voltra_scene::prefab::overrides` for the rules.

use voltra_core::UiFrame;
use voltra_ecs::{Entity, World};
use voltra_scene::{hierarchy, prefab, PrefabInstance, PrefabPart, SceneId};

use crate::editor::Editor;
use crate::undo::SceneView;

/// Refreshes the overrides of every selected entity that is part of an
/// instance — the selection is all the inspector and the gizmo can edit.
/// Called once a frame, before the history takes the frame's after side.
pub(crate) fn refresh_overrides(editor: &mut Editor, frame: &mut UiFrame<'_>) {
    let parts: Vec<Entity> = editor
        .selection
        .iter()
        .filter(|&e| frame.world.is_alive(e) && frame.world.get::<PrefabPart>(e).is_some())
        .collect();
    for entity in parts {
        if let Err(error) =
            prefab::refresh_overrides(frame.world, &editor.registry, frame.prefabs, entity)
        {
            log::error!("could not compare an instance with its prefab: {error}");
        }
    }
}

/// The ids of `root` and everything under it.
fn ids_under(world: &World, root: Entity) -> Vec<SceneId> {
    std::iter::once(root)
        .chain(hierarchy::descendants(world, root))
        .filter_map(|e| world.get::<SceneId>(e).copied())
        .collect()
}

/// Puts `name` on `entity` back to its prefab's — every override, for
/// `None` — as one entry.
pub(crate) fn revert(
    editor: &mut Editor,
    frame: &mut UiFrame<'_>,
    entity: Entity,
    name: Option<&str>,
) {
    let Some(root) = prefab::instance_root(frame.world, entity) else {
        return;
    };
    recorded(
        editor,
        frame,
        "Revert to prefab",
        ids_under(frame.world, root),
        |e, f| {
            prefab::revert(f.world, &e.registry, f.prefabs, entity, name)
                .map_err(|error| error.to_string())
        },
    );
}

/// Puts the whole instance at `root` back to its prefab, as one entry.
pub(crate) fn revert_instance(editor: &mut Editor, frame: &mut UiFrame<'_>, root: Entity) {
    recorded(
        editor,
        frame,
        "Revert instance",
        ids_under(frame.world, root),
        |e, f| {
            prefab::revert_instance(f.world, &e.registry, f.prefabs, root)
                .map_err(|error| error.to_string())
        },
    );
}

/// Writes the instance at `root` over its prefab and brings every instance
/// of it in step, as one entry — of the scene's side of it: the file stays
/// written when the entry is undone, as a created prefab stays.
pub(crate) fn apply(editor: &mut Editor, frame: &mut UiFrame<'_>, root: Entity) {
    let Some(source) = frame
        .world
        .get::<PrefabInstance>(root)
        .map(|l| l.source.clone())
    else {
        return;
    };
    // Every instance of the file changes, not only this one — and every
    // instance of a variant of it, or of a prefab it is nested in.
    let mut sources = vec![source.clone()];
    sources.extend(frame.prefabs.dependents(&source));
    let roots: Vec<Entity> = frame
        .world
        .query::<PrefabInstance>()
        .filter(|(_, link)| sources.contains(&link.source))
        .map(|(e, _)| e)
        .collect();
    let ids: Vec<SceneId> = roots
        .iter()
        .flat_map(|&r| ids_under(frame.world, r))
        .collect();
    recorded(editor, frame, "Apply to prefab", ids, |e, f| {
        prefab::apply(f.world, &e.registry, f.prefabs, root).map_err(|error| error.to_string())
    });
    editor.assets.refresh();
}

/// Runs `act` as the entry `label` over `ids`, resolving the assets of what
/// it touched and recording what it spawned.
fn recorded(
    editor: &mut Editor,
    frame: &mut UiFrame<'_>,
    label: &'static str,
    ids: Vec<SceneId>,
    act: impl FnOnce(&Editor, &mut UiFrame<'_>) -> Result<prefab::SyncReport, String>,
) {
    let view = SceneView {
        world: frame.world,
        registry: &editor.registry,
        selected: editor.selection.ids(frame.world),
    };
    editor.history.begin(label, view, ids);
    let touched = match act(editor, frame) {
        Ok(report) => report.touched,
        Err(error) => {
            log::error!("{}: {error}", label.to_lowercase());
            Vec::new()
        }
    };
    frame.resolve_assets_of(&touched);
    editor.selection.retain_alive(frame.world);
    let spawned: Vec<SceneId> = touched
        .iter()
        .filter_map(|&e| frame.world.get::<SceneId>(e).copied())
        .collect();
    let view = SceneView {
        world: frame.world,
        registry: &editor.registry,
        selected: editor.selection.ids(frame.world),
    };
    editor.history.commit_including(view, spawned);
}
